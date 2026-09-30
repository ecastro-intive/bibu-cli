//! Pull request comment endpoints. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/>

use reqwest::Method;
use serde_json::Value;

use super::models::comment::Comment;
use super::paginate::{fetch, Limit};
use super::Client;
use crate::error::Result;
use crate::repo::RepoRef;

fn base(repo: &RepoRef, pr: u64) -> String {
    format!("{}/pullrequests/{pr}/comments", repo.api_path())
}

/// `GET .../comments`
pub fn list(client: &Client, repo: &RepoRef, pr: u64, limit: Limit) -> Result<Vec<Comment>> {
    fetch(client, &base(repo, pr), &[], limit)
}

/// `POST .../comments`: general, inline, or (with `parent`) a reply.
pub fn create(client: &Client, repo: &RepoRef, pr: u64, body: &Value) -> Result<Comment> {
    client.send(Method::POST, &base(repo, pr), &[], Some(body))
}

/// `PUT .../comments/{id}`
pub fn update(client: &Client, repo: &RepoRef, pr: u64, id: u64, body: &Value) -> Result<Comment> {
    client.send(
        Method::PUT,
        &format!("{}/{id}", base(repo, pr)),
        &[],
        Some(body),
    )
}

/// `DELETE .../comments/{id}`
pub fn delete(client: &Client, repo: &RepoRef, pr: u64, id: u64) -> Result<()> {
    client
        .send::<Value>(
            Method::DELETE,
            &format!("{}/{id}", base(repo, pr)),
            &[],
            None,
        )
        .map(drop)
}

/// `POST .../comments/{id}/resolve`
pub fn resolve(client: &Client, repo: &RepoRef, pr: u64, id: u64) -> Result<()> {
    client
        .send::<Value>(
            Method::POST,
            &format!("{}/{id}/resolve", base(repo, pr)),
            &[],
            None,
        )
        .map(drop)
}

/// `DELETE .../comments/{id}/resolve`
pub fn reopen(client: &Client, repo: &RepoRef, pr: u64, id: u64) -> Result<()> {
    client
        .send::<Value>(
            Method::DELETE,
            &format!("{}/{id}/resolve", base(repo, pr)),
            &[],
            None,
        )
        .map(drop)
}
