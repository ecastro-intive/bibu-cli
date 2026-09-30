//! Branch endpoints. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-refs/>

use reqwest::Method;
use serde_json::{json, Value};

use super::encode_path;
use super::models::branch::{Branch, Repository};
use super::paginate::{fetch_filtered, Limit};
use super::Client;
use crate::error::Result;
use crate::repo::RepoRef;

fn base(repo: &RepoRef) -> String {
    format!("{}/refs/branches", repo.api_path())
}

/// Escapes a value for use inside a Bitbucket `q=` filter string literal.
pub(crate) fn q_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `GET /refs/branches`, most recently updated first. `contains` becomes a server-side
/// `name ~ "..."` filter and is checked again locally.
pub fn list(
    client: &Client,
    repo: &RepoRef,
    contains: Option<&str>,
    limit: Limit,
) -> Result<Vec<Branch>> {
    let mut query = vec![("sort", "-target.date".to_string())];
    if let Some(text) = contains {
        query.push(("q", format!("name ~ \"{}\"", q_literal(text))));
    }
    fetch_filtered(client, &base(repo), &query, limit, |b: &Branch| {
        contains.is_none_or(|t| b.name.to_lowercase().contains(&t.to_lowercase()))
    })
}

/// `GET /refs/branches/{name}`
pub fn get(client: &Client, repo: &RepoRef, name: &str) -> Result<Branch> {
    client.get(&format!("{}/{}", base(repo), encode_path(name)), &[])
}

/// `POST /refs/branches`: a new branch pointing at `hash`.
pub fn create(client: &Client, repo: &RepoRef, name: &str, hash: &str) -> Result<Branch> {
    let body = json!({ "name": name, "target": { "hash": hash } });
    client.send(Method::POST, &base(repo), &[], Some(&body))
}

/// `DELETE /refs/branches/{name}`
pub fn delete(client: &Client, repo: &RepoRef, name: &str) -> Result<()> {
    client
        .send::<Value>(
            Method::DELETE,
            &format!("{}/{}", base(repo), encode_path(name)),
            &[],
            None,
        )
        .map(drop)
}

/// `GET /repositories/{workspace}/{slug}`: used to find the default branch.
pub fn repository(client: &Client, repo: &RepoRef) -> Result<Repository> {
    client.get(&repo.api_path(), &[])
}

#[cfg(test)]
mod tests {
    use super::q_literal;

    #[test]
    fn q_literals_escape_quotes_and_backslashes() {
        assert_eq!(q_literal("plain"), "plain");
        assert_eq!(q_literal(r#"a"b"#), r#"a\"b"#);
        assert_eq!(q_literal(r"a\b"), r"a\\b");
    }
}
