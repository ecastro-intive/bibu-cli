//! `bibu branch list | create | delete`

use comfy_table::presets::NOTHING;
use comfy_table::Table;
use schemars::JsonSchema;
use serde::Serialize;

use super::confirm;
use crate::api::models::branch::Branch;
use crate::api::{branches as api, Client};
use crate::cli::BranchCommand;
use crate::error::{BibuError, Result};
use crate::output::{render, Mode, Render};
use crate::repo::RepoRef;
use crate::terminal::Terminal;

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchRow {
    /// Branch name.
    pub name: String,
    /// Commit at the tip of the branch.
    pub hash: String,
    /// ISO 8601 time of the tip commit.
    pub date: String,
    /// Author of the tip commit.
    pub author: String,
    /// First line of the tip commit's message.
    pub message: String,
    /// Whether this is the repository's default branch.
    pub default: bool,
}

impl BranchRow {
    fn new(branch: &Branch, default_branch: Option<&str>) -> Self {
        let commit = branch.target.as_ref();
        Self {
            name: branch.name.clone(),
            hash: commit.map(|c| c.hash.clone()).unwrap_or_default(),
            date: commit.map(|c| c.date.clone()).unwrap_or_default(),
            author: commit.map(|c| c.author.name()).unwrap_or_default(),
            message: commit.map(|c| c.subject().to_string()).unwrap_or_default(),
            default: default_branch == Some(branch.name.as_str()),
        }
    }
}

fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

impl Render for BranchRow {
    fn render_table(&self) -> String {
        format!(
            "{}{} {} {}",
            self.name,
            if self.default { " (default)" } else { "" },
            self.hash.get(..7).unwrap_or(&self.hash),
            self.message
        )
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct BranchList(pub Vec<BranchRow>);

impl Render for BranchList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No branches found.".to_string();
        }
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["BRANCH", "COMMIT", "AUTHOR", "UPDATED", "MESSAGE"]);
        for b in &self.0 {
            table.add_row(vec![
                if b.default {
                    format!("{} (default)", b.name)
                } else {
                    b.name.clone()
                },
                b.hash.get(..7).unwrap_or(&b.hash).to_string(),
                b.author.clone(),
                day(&b.date).to_string(),
                b.message.clone(),
            ]);
        }
        table.to_string()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchAction {
    /// Branch name.
    pub branch: String,
    /// What happened: `deleted`.
    pub action: &'static str,
}

impl Render for BranchAction {
    fn render_table(&self) -> String {
        format!("Branch {}: {}", self.branch, self.action)
    }
}

/// 4 to 40 hex digits: plausibly a (possibly abbreviated) commit hash.
pub(crate) fn looks_like_commit(value: &str) -> bool {
    (4..=40).contains(&value.len()) && value.chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn validate_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) || name.starts_with('-') {
        return Err(BibuError::Usage(format!(
            "{name:?} is not a valid branch name (no spaces, not empty, no leading dash)"
        )));
    }
    Ok(name)
}

/// The commit to start a new branch from: a branch tip, or a commit hash.
fn start_hash(client: &Client, repo: &RepoRef, from: Option<&str>) -> Result<String> {
    let from = match from {
        Some(from) => from.to_string(),
        None => api::repository(client, repo)?
            .default_branch()
            .map(str::to_string)
            .ok_or_else(|| {
                BibuError::Usage("the repository has no default branch; pass --from".to_string())
            })?,
    };
    match api::get(client, repo, &from) {
        Ok(branch) => branch
            .target
            .map(|c| c.hash)
            .filter(|h| !h.is_empty())
            .ok_or_else(|| BibuError::Other(format!("branch {from:?} has no commit"))),
        Err(BibuError::NotFound(_)) if looks_like_commit(&from) => Ok(from),
        Err(BibuError::NotFound(_)) => Err(BibuError::NotFound(format!(
            "no branch or commit named {from:?}"
        ))),
        Err(other) => Err(other),
    }
}

pub fn run(
    command: &BranchCommand,
    repo: &RepoRef,
    client: &Client,
    term: &dyn Terminal,
    yes: bool,
    mode: Mode,
) -> Result<String> {
    match command {
        BranchCommand::List { name, paging } => {
            let branches = api::list(client, repo, name.as_deref(), paging.limit())?;
            let repository = api::repository(client, repo)?;
            let default = repository.default_branch();
            Ok(render(
                &BranchList(
                    branches
                        .iter()
                        .map(|b| BranchRow::new(b, default))
                        .collect(),
                ),
                mode,
            ))
        }
        BranchCommand::Create { name, from } => {
            let name = validate_name(name)?;
            let hash = start_hash(client, repo, from.as_deref())?;
            let created = api::create(client, repo, name, &hash)?;
            Ok(render(&BranchRow::new(&created, None), mode))
        }
        BranchCommand::Delete { name } => {
            let name = validate_name(name)?;
            if api::repository(client, repo)?.default_branch() == Some(name) {
                return Err(BibuError::Conflict(format!(
                    "refusing to delete {name:?}: it is the repository's default branch"
                )));
            }
            confirm(term, yes, "delete the branch", || {
                Ok(format!("Delete branch {name}?"))
            })?;
            api::delete(client, repo, name)?;
            Ok(render(
                &BranchAction {
                    branch: name.to_string(),
                    action: "deleted",
                },
                mode,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn branch(v: serde_json::Value) -> Branch {
        serde_json::from_value(v).unwrap()
    }

    fn sample() -> Branch {
        branch(json!({
            "name": "feature/x",
            "target": {"hash": "abcdef1234567", "date": "2026-09-30T10:00:00+00:00",
                       "message": "Add x\n\nbody", "author": {"raw": "Jane Doe <j@x.io>"}}
        }))
    }

    #[test]
    fn row_flattens_the_tip_commit() {
        let row = BranchRow::new(&sample(), Some("main"));
        assert_eq!(row.hash, "abcdef1234567");
        assert_eq!(row.author, "Jane Doe");
        assert_eq!(row.message, "Add x");
        assert!(!row.default);
        assert!(BranchRow::new(&sample(), Some("feature/x")).default);
    }

    #[test]
    fn a_branch_without_a_tip_still_renders() {
        let row = BranchRow::new(&branch(json!({"name": "empty"})), None);
        assert_eq!((row.hash.as_str(), row.message.as_str()), ("", ""));
        assert!(row.render_table().starts_with("empty"));
    }

    #[test]
    fn table_marks_the_default_branch_and_shortens_hashes() {
        let text = BranchList(vec![BranchRow::new(&sample(), Some("feature/x"))]).render_table();
        assert!(
            text.contains("feature/x (default)") && text.contains("abcdef1"),
            "{text}"
        );
        assert!(!text.contains("abcdef12"));
        assert_eq!(BranchList(vec![]).render_table(), "No branches found.");
    }

    #[test]
    fn commit_hash_detection() {
        assert!(looks_like_commit("abcd"));
        assert!(looks_like_commit(
            "0123456789abcdef0123456789abcdef01234567"
        ));
        assert!(!looks_like_commit("abc"));
        assert!(!looks_like_commit("main"));
        assert!(!looks_like_commit("feature/x"));
        assert!(!looks_like_commit(&"a".repeat(41)));
    }

    #[test]
    fn branch_names_are_validated() {
        assert_eq!(validate_name("  feature/x ").unwrap(), "feature/x");
        for bad in ["", "  ", "a b", "-x", "a\tb"] {
            assert!(
                matches!(validate_name(bad), Err(BibuError::Usage(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn action_text() {
        assert_eq!(
            BranchAction {
                branch: "x".into(),
                action: "deleted"
            }
            .render_table(),
            "Branch x: deleted"
        );
    }
}
