//! Workspace member endpoint. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-workspaces/>

use super::models::{Account, WorkspaceMember};
use super::paginate::{fetch, Limit};
use super::Client;
use crate::error::Result;

/// `GET /workspaces/{workspace}/members`
pub fn list(client: &Client, workspace: &str, limit: Limit) -> Result<Vec<Account>> {
    let members: Vec<WorkspaceMember> = fetch(
        client,
        &format!("/workspaces/{workspace}/members"),
        &[],
        limit,
    )?;
    Ok(members.into_iter().map(|m| m.user).collect())
}
