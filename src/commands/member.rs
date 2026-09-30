//! `bibu member list | find`, plus name-to-account lookup used by `pr reviewers`.

use comfy_table::presets::NOTHING;
use comfy_table::Table;
use schemars::JsonSchema;
use serde::Serialize;

use crate::api::models::Account;
use crate::api::{members, user, Client, Limit};
use crate::cli::MemberCommand;
use crate::error::{BibuError, Result};
use crate::output::{render, Mode, Render};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct MemberList(pub Vec<Account>);

impl Render for MemberList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No members found.".to_string();
        }
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["NAME", "NICKNAME", "UUID"]);
        for a in &self.0 {
            table.add_row(vec![
                a.display_name.clone(),
                a.nickname.clone().unwrap_or_default(),
                a.uuid.clone(),
            ]);
        }
        table.to_string()
    }
}

pub fn run(
    command: &MemberCommand,
    workspace: &str,
    client: &Client,
    mode: Mode,
) -> Result<String> {
    match command {
        MemberCommand::List { paging } => Ok(render(
            &MemberList(members::list(client, workspace, paging.limit())?),
            mode,
        )),
        MemberCommand::Find { query } => {
            let all = members::list(client, workspace, Limit::All)?;
            Ok(render(&MemberList(find(&all, query)), mode))
        }
    }
}

fn eq(a: &str, b: &str) -> bool {
    !a.is_empty() && a.eq_ignore_ascii_case(b)
}

fn is_exact(account: &Account, query: &str) -> bool {
    eq(&account.uuid, query)
        || account
            .account_id
            .as_deref()
            .is_some_and(|id| eq(id, query))
        || account.nickname.as_deref().is_some_and(|n| eq(n, query))
        || eq(&account.display_name, query)
}

fn is_partial(account: &Account, query: &str) -> bool {
    let q = query.to_lowercase();
    account.display_name.to_lowercase().contains(&q)
        || account
            .nickname
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains(&q))
}

/// Everyone matching `query`: exact matches (uuid, account id, nickname, name) win outright;
/// otherwise case-insensitive substring matches on name and nickname.
pub fn find(accounts: &[Account], query: &str) -> Vec<Account> {
    let exact: Vec<_> = accounts
        .iter()
        .filter(|a| is_exact(a, query))
        .cloned()
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    accounts
        .iter()
        .filter(|a| is_partial(a, query))
        .cloned()
        .collect()
}

/// Exactly one person from `candidates`, or a helpful error.
pub fn pick(candidates: &[Account], query: &str, what: &str) -> Result<Account> {
    let matches = find(candidates, query);
    match matches.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(BibuError::NotFound(format!("no {what} matches {query:?}"))),
        many => Err(BibuError::Usage(format!(
            "{query:?} matches {} people: {}; be more specific or use the uuid",
            many.len(),
            many.iter()
                .map(|a| format!("{} ({})", a.display_name, a.uuid))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn looks_like_uuid(query: &str) -> bool {
    query.len() > 2 && query.starts_with('{') && query.ends_with('}')
}

/// Turns what a person typed into an account with a uuid: `me`, `{uuid}`, or a lookup among
/// the workspace members.
pub fn resolve_user(client: &Client, workspace: &str, query: &str) -> Result<Account> {
    if query.eq_ignore_ascii_case("me") {
        return user::current(client);
    }
    if looks_like_uuid(query) {
        return Ok(Account {
            uuid: query.to_string(),
            ..Account::default()
        });
    }
    let all = members::list(client, workspace, Limit::All)?;
    pick(&all, query, "workspace member")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(name: &str, nick: &str, uuid: &str, account_id: &str) -> Account {
        Account {
            display_name: name.into(),
            uuid: uuid.into(),
            nickname: Some(nick.into()),
            account_id: Some(account_id.into()),
        }
    }

    fn team() -> Vec<Account> {
        vec![
            acct("Jane Doe", "jane", "{j}", "712020:jane"),
            acct("Jane Dorsey", "jdorsey", "{jd}", "712020:jd"),
            acct("Bob Ray", "bob", "{b}", "712020:bob"),
        ]
    }

    #[test]
    fn exact_matches_win_over_partial_ones() {
        let found = find(&team(), "jane");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].uuid, "{j}");
    }

    #[test]
    fn every_identifier_kind_matches_case_insensitively() {
        for q in [
            "Jane Doe",
            "JANE DOE",
            "jane",
            "{j}",
            "712020:jane",
            "712020:JANE",
        ] {
            let found = find(&team(), q);
            assert_eq!(found.len(), 1, "{q}");
            assert_eq!(found[0].uuid, "{j}", "{q}");
        }
    }

    #[test]
    fn partial_matches_are_a_fallback() {
        assert_eq!(find(&team(), "dors").len(), 1);
        assert_eq!(find(&team(), "ja").len(), 2);
        assert!(find(&team(), "zzz").is_empty());
    }

    #[test]
    fn empty_fields_never_match_an_empty_query_exactly() {
        let blank = Account::default();
        assert!(!is_exact(&blank, ""));
    }

    #[test]
    fn pick_returns_the_single_match() {
        assert_eq!(pick(&team(), "bob", "reviewer").unwrap().uuid, "{b}");
    }

    #[test]
    fn pick_reports_no_match_as_not_found() {
        let err = pick(&team(), "zzz", "workspace member").unwrap_err();
        assert!(matches!(&err, BibuError::NotFound(m) if m.contains("workspace member")));
    }

    #[test]
    fn pick_lists_candidates_when_ambiguous() {
        let err = pick(&team(), "ja", "workspace member").unwrap_err();
        let BibuError::Usage(message) = err else {
            panic!("expected a usage error")
        };
        assert!(message.contains("Jane Doe") && message.contains("Jane Dorsey"));
        assert!(message.contains("{jd}"));
    }

    #[test]
    fn uuid_shape_detection() {
        assert!(looks_like_uuid("{abc-123}"));
        assert!(!looks_like_uuid("abc"));
        assert!(!looks_like_uuid("{}"));
    }

    #[test]
    fn table_lists_name_nickname_and_uuid() {
        let text = MemberList(team()).render_table();
        assert!(text.contains("Jane Doe") && text.contains("jdorsey") && text.contains("{b}"));
        assert_eq!(MemberList(vec![]).render_table(), "No members found.");
    }
}
