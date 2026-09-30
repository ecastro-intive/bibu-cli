//! Pull request endpoints. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/>

use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

use super::models::pullrequest::{Commit, DiffStat, PullRequest};
use super::models::Account;
use super::paginate::{fetch, fetch_filtered, Limit};
use super::{user, Client};
use crate::error::Result;
use crate::repo::RepoRef;

fn pr_path(repo: &RepoRef, id: u64, suffix: &str) -> String {
    format!("{}/pullrequests/{id}{suffix}", repo.api_path())
}

/// `GET /pullrequests?state=...`. The list payload omits reviewers and participants.
pub fn list(
    client: &Client,
    repo: &RepoRef,
    state: &str,
    limit: Limit,
    keep: impl Fn(&PullRequest) -> bool,
) -> Result<Vec<PullRequest>> {
    let path = format!("{}/pullrequests", repo.api_path());
    fetch_filtered(client, &path, &[("state", state.to_string())], limit, keep)
}

/// `GET /pullrequests/{id}`: the only call that includes reviewers and participants.
pub fn get(client: &Client, repo: &RepoRef, id: u64) -> Result<PullRequest> {
    client.get(&pr_path(repo, id, ""), &[])
}

/// `POST /pullrequests`
pub fn create(client: &Client, repo: &RepoRef, body: &Value) -> Result<PullRequest> {
    let path = format!("{}/pullrequests", repo.api_path());
    client.send(Method::POST, &path, &[], Some(body))
}

/// `PUT /pullrequests/{id}`: only the keys present in `body` change.
pub fn update(client: &Client, repo: &RepoRef, id: u64, body: &Value) -> Result<PullRequest> {
    client.send(Method::PUT, &pr_path(repo, id, ""), &[], Some(body))
}

/// `POST /approve`
pub fn approve(client: &Client, repo: &RepoRef, id: u64) -> Result<()> {
    client
        .send::<Value>(Method::POST, &pr_path(repo, id, "/approve"), &[], None)
        .map(drop)
}

/// `DELETE /approve`
pub fn unapprove(client: &Client, repo: &RepoRef, id: u64) -> Result<()> {
    client
        .send::<Value>(Method::DELETE, &pr_path(repo, id, "/approve"), &[], None)
        .map(drop)
}

/// `POST /request-changes`
pub fn request_changes(client: &Client, repo: &RepoRef, id: u64) -> Result<()> {
    client
        .send::<Value>(
            Method::POST,
            &pr_path(repo, id, "/request-changes"),
            &[],
            None,
        )
        .map(drop)
}

/// `DELETE /request-changes`
pub fn unrequest_changes(client: &Client, repo: &RepoRef, id: u64) -> Result<()> {
    client
        .send::<Value>(
            Method::DELETE,
            &pr_path(repo, id, "/request-changes"),
            &[],
            None,
        )
        .map(drop)
}

/// `POST /merge` (synchronous). `body` may carry `merge_strategy`, `message`,
/// `close_source_branch`.
pub fn merge(client: &Client, repo: &RepoRef, id: u64, body: &Value) -> Result<PullRequest> {
    client.send(Method::POST, &pr_path(repo, id, "/merge"), &[], Some(body))
}

/// `POST /decline`
pub fn decline(client: &Client, repo: &RepoRef, id: u64) -> Result<PullRequest> {
    client.send(
        Method::POST,
        &pr_path(repo, id, "/decline"),
        &[],
        Some(&json!({})),
    )
}

/// `GET /diff`: a unified diff (Bitbucket redirects to the repository diff).
pub fn diff(client: &Client, repo: &RepoRef, id: u64) -> Result<String> {
    client.get_text(&pr_path(repo, id, "/diff"), &[])
}

/// `GET /diffstat`
pub fn diffstat(client: &Client, repo: &RepoRef, id: u64, limit: Limit) -> Result<Vec<DiffStat>> {
    fetch(client, &pr_path(repo, id, "/diffstat"), &[], limit)
}

/// `GET /commits`
pub fn commits(client: &Client, repo: &RepoRef, id: u64, limit: Limit) -> Result<Vec<Commit>> {
    fetch(client, &pr_path(repo, id, "/commits"), &[], limit)
}

#[derive(Deserialize)]
struct DefaultReviewer {
    user: Account,
}

/// The repository's effective default reviewers, minus the current user (an author cannot
/// review their own pull request).
pub fn default_reviewers(client: &Client, repo: &RepoRef) -> Result<Vec<Account>> {
    let me = user::current(client)?;
    let path = format!("{}/effective-default-reviewers", repo.api_path());
    let all: Vec<DefaultReviewer> = fetch(client, &path, &[], Limit::All)?;
    Ok(all
        .into_iter()
        .map(|r| r.user)
        .filter(|u| u.uuid != me.uuid)
        .collect())
}
