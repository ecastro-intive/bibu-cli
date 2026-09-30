//! Repository resolution: which `workspace/slug` are we talking about?
//!
//! Precedence: `--repo` flag, then `BIBU_REPO`, then the git `origin` remote.

use std::fmt;
use std::process::Command;

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::{BibuError, Result};
use crate::output::Render;

const BITBUCKET_HOSTS: [&str; 2] = ["bitbucket.org", "www.bitbucket.org"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RepoRef {
    /// Workspace slug.
    pub workspace: String,
    /// Repository slug.
    pub slug: String,
}

impl fmt::Display for RepoRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.workspace, self.slug)
    }
}

impl Render for RepoRef {
    fn render_table(&self) -> String {
        self.to_string()
    }
}

impl RepoRef {
    /// Path prefix for repository-scoped API endpoints.
    pub fn api_path(&self) -> String {
        format!("/repositories/{}/{}", self.workspace, self.slug)
    }
}

/// Parses `ws/slug` shorthand or any Bitbucket remote URL (HTTPS, `ssh://`, scp-like).
pub fn parse(input: &str) -> Result<RepoRef> {
    let input = input.trim();
    let invalid = || {
        BibuError::Usage(format!(
            "cannot read a Bitbucket repository from {input:?}; \
             expected \"workspace/repo\" or a bitbucket.org URL"
        ))
    };

    let path = match split_host_path(input) {
        Some((host, path)) => {
            if !BITBUCKET_HOSTS.contains(&host.to_ascii_lowercase().as_str()) {
                return Err(invalid());
            }
            path
        }
        None => input.to_string(),
    };

    let mut segments = path.split('/').filter(|s| !s.is_empty());
    let (Some(workspace), Some(slug)) = (segments.next(), segments.next()) else {
        return Err(invalid());
    };
    let slug = slug.strip_suffix(".git").unwrap_or(slug);

    if !is_valid_name(workspace) || !is_valid_name(slug) {
        return Err(invalid());
    }
    Ok(RepoRef {
        workspace: workspace.to_string(),
        slug: slug.to_string(),
    })
}

fn is_valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Splits a URL into `(host, path)`. Returns `None` for plain `ws/slug` shorthand.
fn split_host_path(s: &str) -> Option<(String, String)> {
    if let Some((_, rest)) = s.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host = host_port.split(':').next().unwrap_or(host_port);
        return Some((host.to_string(), path.to_string()));
    }
    // scp-like: [user@]host:path
    if let Some((left, path)) = s.split_once(':') {
        let host = left.rsplit_once('@').map_or(left, |(_, h)| h);
        return Some((host.to_string(), path.to_string()));
    }
    None
}

/// Pure resolution logic; the git lookup is injected so it can be tested.
pub fn resolve_with(
    flag: Option<&str>,
    env: Option<&str>,
    git_remote: impl FnOnce() -> Option<String>,
) -> Result<RepoRef> {
    if let Some(value) = flag.filter(|v| !v.trim().is_empty()) {
        return parse(value);
    }
    if let Some(value) = env.filter(|v| !v.trim().is_empty()) {
        return parse(value);
    }
    match git_remote() {
        Some(url) => parse(&url),
        None => Err(BibuError::Usage(
            "no repository given and no git `origin` remote found; \
             pass --repo workspace/repo or set BIBU_REPO"
                .to_string(),
        )),
    }
}

pub fn resolve(flag: Option<&str>) -> Result<RepoRef> {
    let env = std::env::var("BIBU_REPO").ok();
    resolve_with(flag, env.as_deref(), git_origin_url)
}

/// The checked-out git branch, or `None` outside a repository / on a detached HEAD.
pub fn current_branch() -> Option<String> {
    let out = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let branch = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

fn git_origin_url() -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!url.is_empty()).then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(ws: &str, slug: &str) -> RepoRef {
        RepoRef {
            workspace: ws.into(),
            slug: slug.into(),
        }
    }

    #[test]
    fn parses_shorthand() {
        assert_eq!(parse("acme/api").unwrap(), repo("acme", "api"));
        assert_eq!(parse("  acme/api  ").unwrap(), repo("acme", "api"));
        assert_eq!(
            parse("my_ws/my.repo-x").unwrap(),
            repo("my_ws", "my.repo-x")
        );
    }

    #[test]
    fn parses_https_urls() {
        for url in [
            "https://bitbucket.org/acme/api",
            "https://bitbucket.org/acme/api.git",
            "https://bitbucket.org/acme/api/",
            "https://jdoe@bitbucket.org/acme/api.git",
            "https://jdoe:secret@bitbucket.org/acme/api.git",
            "https://www.bitbucket.org/acme/api",
            "https://bitbucket.org/acme/api/pull-requests/12",
            "HTTPS://BITBUCKET.ORG/acme/api",
        ] {
            assert_eq!(parse(url).unwrap(), repo("acme", "api"), "{url}");
        }
    }

    #[test]
    fn parses_ssh_urls() {
        for url in [
            "git@bitbucket.org:acme/api.git",
            "git@bitbucket.org:acme/api",
            "ssh://git@bitbucket.org/acme/api.git",
            "ssh://git@bitbucket.org:22/acme/api.git",
        ] {
            assert_eq!(parse(url).unwrap(), repo("acme", "api"), "{url}");
        }
    }

    #[test]
    fn keeps_dots_inside_slug_but_strips_git_suffix() {
        assert_eq!(parse("acme/my.repo.git").unwrap(), repo("acme", "my.repo"));
    }

    #[test]
    fn rejects_non_bitbucket_hosts() {
        for url in ["https://github.com/acme/api", "git@github.com:acme/api.git"] {
            assert!(matches!(parse(url), Err(BibuError::Usage(_))), "{url}");
        }
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "acme",
            "acme/",
            "/api",
            "https://bitbucket.org/acme",
            "a b/c",
            "a/b c",
        ] {
            assert!(matches!(parse(bad), Err(BibuError::Usage(_))), "{bad:?}");
        }
    }

    #[test]
    fn flag_beats_env_beats_git() {
        let git = || Some("git@bitbucket.org:git/remote.git".to_string());
        assert_eq!(
            resolve_with(Some("f/flag"), Some("e/env"), git).unwrap(),
            repo("f", "flag")
        );
        assert_eq!(
            resolve_with(None, Some("e/env"), git).unwrap(),
            repo("e", "env")
        );
        assert_eq!(
            resolve_with(None, None, git).unwrap(),
            repo("git", "remote")
        );
    }

    #[test]
    fn blank_flag_and_env_fall_through() {
        let git = || Some("https://bitbucket.org/g/r.git".to_string());
        assert_eq!(
            resolve_with(Some("  "), Some(""), git).unwrap(),
            repo("g", "r")
        );
    }

    #[test]
    fn invalid_flag_is_an_error_not_a_silent_fallback() {
        let git = || Some("https://bitbucket.org/g/r.git".to_string());
        assert!(resolve_with(Some("nonsense"), None, git).is_err());
    }

    #[test]
    fn missing_everything_explains_how_to_fix() {
        let err = resolve_with(None, None, || None).unwrap_err();
        assert!(err.to_string().contains("--repo"));
    }

    #[test]
    fn api_path_is_repository_scoped() {
        assert_eq!(repo("acme", "api").api_path(), "/repositories/acme/api");
    }
}
