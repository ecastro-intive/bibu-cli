//! Pipeline endpoints. Reference:
//! <https://developer.atlassian.com/cloud/bitbucket/rest/api-group-pipelines/>

use super::encode_path;
use super::models::pipeline::{Pipeline, Step};
use super::paginate::{fetch, fetch_filtered, Limit};
use super::Client;
use crate::error::{BibuError, Result};
use crate::repo::RepoRef;

fn base(repo: &RepoRef) -> String {
    format!("{}/pipelines", repo.api_path())
}

/// `GET /pipelines`, newest first. `branch` is sent to the server and checked again locally;
/// `keep` decides what else to drop before the limit counts.
pub fn list(
    client: &Client,
    repo: &RepoRef,
    branch: Option<&str>,
    limit: Limit,
    keep: impl Fn(&Pipeline) -> bool,
) -> Result<Vec<Pipeline>> {
    let mut query = vec![("sort", "-created_on".to_string())];
    if let Some(branch) = branch {
        query.push(("target.branch", branch.to_string()));
    }
    fetch_filtered(client, &base(repo), &query, limit, |p: &Pipeline| {
        branch.is_none_or(|b| p.target.ref_name == b) && keep(p)
    })
}

/// `GET /pipelines/{id}` where `id` is a uuid (`{...}`) or a build number.
pub fn get(client: &Client, repo: &RepoRef, id: &str) -> Result<Pipeline> {
    client.get(&format!("{}/{}", base(repo), encode_path(id)), &[])
}

/// Finds a pipeline by build number. Bitbucket documents only uuids in this slot, so when the
/// direct lookup misses, fall back to scanning the list (newest first) for that number.
pub fn get_by_build_number(client: &Client, repo: &RepoRef, number: u64) -> Result<Pipeline> {
    match get(client, repo, &number.to_string()) {
        Err(BibuError::NotFound(_)) => {}
        other => return other,
    }
    let found = fetch_filtered(
        client,
        &base(repo),
        &[("sort", "-created_on".to_string())],
        Limit::All,
        |p: &Pipeline| p.build_number == number,
    )?;
    found
        .into_iter()
        .next()
        .ok_or_else(|| BibuError::NotFound(format!("no pipeline with build number {number}")))
}

/// `GET /pipelines/{id}/steps`
pub fn steps(client: &Client, repo: &RepoRef, pipeline_uuid: &str) -> Result<Vec<Step>> {
    let path = format!("{}/{}/steps", base(repo), encode_path(pipeline_uuid));
    fetch(client, &path, &[], Limit::All)
}

/// `GET /pipelines/{id}/steps/{step}/log`: the raw log text. Finished logs are served from
/// long-term storage through a redirect, which the client follows.
pub fn step_log(
    client: &Client,
    repo: &RepoRef,
    pipeline_uuid: &str,
    step_uuid: &str,
) -> Result<String> {
    let path = format!(
        "{}/{}/steps/{}/log",
        base(repo),
        encode_path(pipeline_uuid),
        encode_path(step_uuid)
    );
    client.get_text(&path, &[])
}
