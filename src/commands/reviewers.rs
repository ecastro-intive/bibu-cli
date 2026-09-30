//! `bibu pr reviewers list | add | remove`
//!
//! Bitbucket only changes reviewers through a full-list `PUT` of the pull request, so `add` and
//! `remove` read the current list, compute the new one, and write it back.

use serde::Serialize;
use serde_json::json;

use super::member::{pick, resolve_user};
use super::pr::ReviewerState;
use crate::api::models::Account;
use crate::api::{pullrequests as api, Client};
use crate::cli::ReviewersCommand;
use crate::error::Result;
use crate::output::{render, Mode, Render};
use crate::repo::RepoRef;

#[derive(Debug, Serialize)]
pub struct ReviewerList {
    pub pull_request: u64,
    pub reviewers: Vec<ReviewerState>,
}

impl Render for ReviewerList {
    fn render_table(&self) -> String {
        if self.reviewers.is_empty() {
            return format!("PR #{} has no reviewers.", self.pull_request);
        }
        let mut lines = vec![format!("Reviewers of PR #{}:", self.pull_request)];
        lines.extend(
            self.reviewers
                .iter()
                .map(|r| format!("  {} - {}", r.user.display_name, r.label())),
        );
        lines.join("\n")
    }
}

/// The current reviewers with their review state.
fn fetch(client: &Client, repo: &RepoRef, id: u64) -> Result<(Vec<Account>, ReviewerList)> {
    let pr = api::get(client, repo, id)?;
    let reviewers = pr
        .participants
        .iter()
        .filter(|p| p.role == "REVIEWER")
        .map(ReviewerState::from)
        .collect();
    Ok((
        pr.reviewers.clone(),
        ReviewerList {
            pull_request: id,
            reviewers,
        },
    ))
}

/// `current` followed by every account in `add` that is not already there (by uuid).
pub(crate) fn merged(current: &[Account], add: &[Account]) -> Vec<Account> {
    let mut out: Vec<Account> = current.to_vec();
    for account in add {
        if !out.iter().any(|a| a.uuid == account.uuid) {
            out.push(account.clone());
        }
    }
    out
}

/// `current` without the accounts in `remove` (by uuid).
pub(crate) fn without(current: &[Account], remove: &[Account]) -> Vec<Account> {
    current
        .iter()
        .filter(|a| !remove.iter().any(|r| r.uuid == a.uuid))
        .cloned()
        .collect()
}

pub(crate) fn payload(reviewers: &[Account]) -> serde_json::Value {
    json!({ "reviewers": reviewers.iter().map(|r| json!({ "uuid": r.uuid })).collect::<Vec<_>>() })
}

pub fn run(
    command: &ReviewersCommand,
    repo: &RepoRef,
    client: &Client,
    mode: Mode,
) -> Result<String> {
    match command {
        ReviewersCommand::List { id } => Ok(render(&fetch(client, repo, *id)?.1, mode)),
        ReviewersCommand::Add { id, users } => {
            let (current, _) = fetch(client, repo, *id)?;
            let mut wanted = Vec::new();
            for user in users {
                wanted.push(resolve_user(client, &repo.workspace, user)?);
            }
            api::update(client, repo, *id, &payload(&merged(&current, &wanted)))?;
            Ok(render(&fetch(client, repo, *id)?.1, mode))
        }
        ReviewersCommand::Remove { id, users } => {
            let (current, _) = fetch(client, repo, *id)?;
            let mut unwanted = Vec::new();
            for user in users {
                unwanted.push(pick(&current, user, "current reviewer")?);
            }
            api::update(client, repo, *id, &payload(&without(&current, &unwanted)))?;
            Ok(render(&fetch(client, repo, *id)?.1, mode))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(name: &str, uuid: &str) -> Account {
        Account {
            display_name: name.into(),
            uuid: uuid.into(),
            ..Account::default()
        }
    }

    #[test]
    fn merged_keeps_existing_order_and_appends_new_people() {
        let out = merged(&[acct("A", "{a}"), acct("B", "{b}")], &[acct("C", "{c}")]);
        assert_eq!(
            out.iter().map(|a| a.uuid.as_str()).collect::<Vec<_>>(),
            ["{a}", "{b}", "{c}"]
        );
    }

    #[test]
    fn merged_never_duplicates_anyone() {
        let out = merged(
            &[acct("A", "{a}")],
            &[acct("A again", "{a}"), acct("C", "{c}"), acct("C", "{c}")],
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].display_name, "A");
    }

    #[test]
    fn without_removes_only_the_named_people() {
        let out = without(
            &[acct("A", "{a}"), acct("B", "{b}"), acct("C", "{c}")],
            &[acct("B", "{b}")],
        );
        assert_eq!(
            out.iter().map(|a| a.uuid.as_str()).collect::<Vec<_>>(),
            ["{a}", "{c}"]
        );
        assert_eq!(without(&[acct("A", "{a}")], &[acct("Z", "{z}")]).len(), 1);
    }

    #[test]
    fn removing_everyone_yields_an_empty_list() {
        assert!(without(&[acct("A", "{a}")], &[acct("A", "{a}")]).is_empty());
    }

    #[test]
    fn payload_sends_uuids_only() {
        assert_eq!(
            payload(&[acct("A", "{a}")]),
            json!({"reviewers": [{"uuid": "{a}"}]})
        );
        assert_eq!(payload(&[]), json!({"reviewers": []}));
    }

    #[test]
    fn list_table_shows_state_or_an_empty_message() {
        let empty = ReviewerList {
            pull_request: 3,
            reviewers: vec![],
        };
        assert_eq!(empty.render_table(), "PR #3 has no reviewers.");
        let one = ReviewerList {
            pull_request: 3,
            reviewers: vec![ReviewerState {
                user: acct("Bob", "{b}"),
                role: "REVIEWER".into(),
                approved: false,
                state: Some("changes_requested".into()),
            }],
        };
        assert!(one.render_table().contains("Bob - changes requested"));
    }
}
