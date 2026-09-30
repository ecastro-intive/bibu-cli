//! Pull request task endpoints. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pullrequests/>

use reqwest::Method;
use serde_json::Value;

use super::models::task::Task;
use super::paginate::{fetch, Limit};
use super::Client;
use crate::error::Result;
use crate::repo::RepoRef;

fn base(repo: &RepoRef, pr: u64) -> String {
    format!("{}/pullrequests/{pr}/tasks", repo.api_path())
}

/// `GET .../tasks`
pub fn list(client: &Client, repo: &RepoRef, pr: u64, limit: Limit) -> Result<Vec<Task>> {
    fetch(client, &base(repo, pr), &[], limit)
}

/// `GET .../tasks/{id}`
pub fn get(client: &Client, repo: &RepoRef, pr: u64, id: u64) -> Result<Task> {
    client.get(&format!("{}/{id}", base(repo, pr)), &[])
}

/// `POST .../tasks`
pub fn create(client: &Client, repo: &RepoRef, pr: u64, body: &Value) -> Result<Task> {
    client.send(Method::POST, &base(repo, pr), &[], Some(body))
}

/// `PUT .../tasks/{id}`
pub fn update(client: &Client, repo: &RepoRef, pr: u64, id: u64, body: &Value) -> Result<Task> {
    client.send(
        Method::PUT,
        &format!("{}/{id}", base(repo, pr)),
        &[],
        Some(body),
    )
}
