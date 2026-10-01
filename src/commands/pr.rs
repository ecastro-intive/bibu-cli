//! `bibu pr ...`

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::api::models::pullrequest::{Commit, DiffStat, Participant, PullRequest};
use crate::api::models::Account;
use crate::api::{pullrequests as api, user};
use crate::cli::{Cli, MergeStrategy, PrCommand};
use crate::context::Context;
use crate::error::{BibuError, Result};
use crate::output::table::{self, Column, NARROW, TINY};
use crate::output::{render, Mode, Render};
use crate::repo;
use crate::terminal::Terminal;

use super::confirm;

// ---------------------------------------------------------------- output types

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReviewerState {
    /// The person.
    pub user: Account,
    /// `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved.
    pub role: String,
    /// Whether they approved.
    pub approved: bool,
    /// `approved`, `changes_requested`, or null while pending / commented only.
    pub state: Option<String>,
}

impl From<&Participant> for ReviewerState {
    fn from(p: &Participant) -> Self {
        Self {
            user: p.user.clone(),
            role: p.role.clone(),
            approved: p.approved,
            state: p.state.clone(),
        }
    }
}

impl ReviewerState {
    pub(crate) fn label(&self) -> &str {
        match self.state.as_deref() {
            Some("approved") => "approved",
            Some("changes_requested") => "changes requested",
            _ if self.approved => "approved",
            _ if self.role == "REVIEWER" => "pending",
            _ => "commented",
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PrSummary {
    /// Pull request number.
    pub id: u64,
    /// Title.
    pub title: String,
    /// `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`.
    pub state: String,
    /// Whether it is a draft.
    pub draft: bool,
    /// Who opened it.
    pub author: Option<Account>,
    /// Branch the changes come from.
    pub source_branch: String,
    /// Branch the changes go into.
    pub destination_branch: String,
    /// Number of comments.
    pub comment_count: u64,
    /// Number of open tasks.
    pub task_count: u64,
    /// ISO 8601 creation time.
    pub created_on: String,
    /// ISO 8601 time of the last update.
    pub updated_on: String,
    /// Link to the pull request in Bitbucket.
    pub url: String,
    /// Present for `view` and `list --reviews`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<Vec<ReviewerState>>,
}

impl PrSummary {
    pub(crate) fn new(pr: &PullRequest, with_participants: bool) -> Self {
        Self {
            id: pr.id,
            title: pr.title.clone(),
            state: pr.state.clone(),
            draft: pr.draft,
            author: pr.author.clone(),
            source_branch: pr.source.branch_name(),
            destination_branch: pr.destination.branch_name(),
            comment_count: pr.comment_count,
            task_count: pr.task_count,
            created_on: pr.created_on.clone(),
            updated_on: pr.updated_on.clone(),
            url: pr.url(),
            participants: with_participants
                .then(|| pr.participants.iter().map(ReviewerState::from).collect()),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct PrList(pub Vec<PrSummary>);

/// `YYYY-MM-DD` from an ISO timestamp.
fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

fn author_name(author: &Option<Account>) -> &str {
    author.as_ref().map_or("", |a| a.display_name.as_str())
}

fn reviews_summary(participants: &[ReviewerState]) -> String {
    participants
        .iter()
        .filter(|p| p.role == "REVIEWER" || p.state.is_some() || p.approved)
        .map(|p| format!("{} ({})", p.user.display_name, p.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Render for PrList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No pull requests found.".to_string();
        }
        let with_reviews = self.0.iter().any(|p| p.participants.is_some());
        let mut columns = vec![
            Column::keep("ID"),
            Column::keep("STATE"),
            Column::flex("TITLE"),
            Column::flex_below("AUTHOR", TINY),
            Column::flex("BRANCH"),
            Column::keep_below("UPDATED", NARROW),
        ];
        if with_reviews {
            columns.push(Column::flex("REVIEWS"));
        }
        let rows = self
            .0
            .iter()
            .map(|pr| {
                let title = if pr.draft {
                    format!("[draft] {}", pr.title)
                } else {
                    pr.title.clone()
                };
                let mut row = vec![
                    format!("#{}", pr.id),
                    pr.state.clone(),
                    title,
                    author_name(&pr.author).to_string(),
                    format!("{} -> {}", pr.source_branch, pr.destination_branch),
                    day(&pr.updated_on).to_string(),
                ];
                if with_reviews {
                    row.push(reviews_summary(pr.participants.as_deref().unwrap_or(&[])));
                }
                row
            })
            .collect();
        table::render(&columns, rows)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PrDetail {
    /// The summary fields (flattened into this object).
    #[serde(flatten)]
    pub summary: PrSummary,
    /// Description (markdown).
    pub description: String,
    /// Whether the source branch is deleted when the pull request is merged.
    pub close_source_branch: bool,
    /// Hash of the merge commit, once merged.
    pub merge_commit: Option<String>,
    /// Why it was declined, when it was.
    pub reason: Option<String>,
}

impl PrDetail {
    pub(crate) fn new(pr: &PullRequest) -> Self {
        Self {
            summary: PrSummary::new(pr, true),
            description: pr.description_text(),
            close_source_branch: pr.close_source_branch,
            merge_commit: pr.merge_commit.as_ref().map(|c| c.hash.clone()),
            reason: pr.reason.clone().filter(|r| !r.is_empty()),
        }
    }
}

impl Render for PrDetail {
    fn render_table(&self) -> String {
        let s = &self.summary;
        let mut lines = vec![
            format!("#{} {}", s.id, s.title),
            format!(
                "{}{} | {} -> {}",
                s.state,
                if s.draft { ", draft" } else { "" },
                s.source_branch,
                s.destination_branch
            ),
            format!(
                "Author: {} | created {} | updated {}",
                author_name(&s.author),
                day(&s.created_on),
                day(&s.updated_on)
            ),
            format!(
                "Comments: {} | open tasks: {}",
                s.comment_count, s.task_count
            ),
            s.url.clone(),
        ];
        if let Some(commit) = &self.merge_commit {
            lines.push(format!("Merge commit: {commit}"));
        }
        if let Some(reason) = &self.reason {
            lines.push(format!("Reason: {reason}"));
        }
        if let Some(participants) = &s.participants {
            if !participants.is_empty() {
                lines.push(String::new());
                lines.push("Reviewers:".to_string());
                for p in participants {
                    lines.push(format!("  {} - {}", p.user.display_name, p.label()));
                }
            }
        }
        if !self.description.trim().is_empty() {
            lines.push(String::new());
            lines.push(self.description.trim_end().to_string());
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Created {
    /// The pull requests that were created, one per destination branch.
    pub pull_requests: Vec<PrSummary>,
}

impl Render for Created {
    fn render_table(&self) -> String {
        self.pull_requests
            .iter()
            .map(|pr| {
                format!(
                    "Created #{} {} ({} -> {})\n{}",
                    pr.id, pr.title, pr.source_branch, pr.destination_branch, pr.url
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ActionResult {
    /// Pull request number.
    pub pull_request: u64,
    /// What happened: `approved`, `approval_removed`, `changes_requested` or `change_request_removed`.
    pub action: &'static str,
}

impl Render for ActionResult {
    fn render_table(&self) -> String {
        format!(
            "PR #{}: {}",
            self.pull_request,
            self.action.replace('_', " ")
        )
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DiffResult {
    /// Pull request number.
    pub pull_request: u64,
    /// The unified diff.
    pub diff: String,
}

impl Render for DiffResult {
    fn render_table(&self) -> String {
        self.diff.trim_end_matches('\n').to_string()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitRow {
    /// Full commit hash.
    pub hash: String,
    /// Author's display name.
    pub author: String,
    /// ISO 8601 commit time.
    pub date: String,
    /// First line of the commit message.
    pub subject: String,
    /// Link to the commit in Bitbucket.
    pub url: String,
}

impl From<&Commit> for CommitRow {
    fn from(c: &Commit) -> Self {
        Self {
            hash: c.hash.clone(),
            author: c.author.name(),
            date: c.date.clone(),
            subject: c.subject().to_string(),
            url: c
                .links
                .html
                .as_ref()
                .map(|l| l.href.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct CommitList(pub Vec<CommitRow>);

impl Render for CommitList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No commits.".to_string();
        }
        const COLUMNS: [Column; 4] = [
            Column::keep("HASH"),
            Column::flex_below("AUTHOR", TINY),
            Column::keep_below("DATE", NARROW),
            Column::flex("SUBJECT"),
        ];
        let rows = self
            .0
            .iter()
            .map(|c| {
                vec![
                    c.hash.get(..7).unwrap_or(&c.hash).to_string(),
                    c.author.clone(),
                    day(&c.date).to_string(),
                    c.subject.clone(),
                ]
            })
            .collect();
        table::render(&COLUMNS, rows)
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileRow {
    /// Path of the changed file (the old path for removed files).
    pub path: String,
    /// `added`, `modified`, `removed` or `renamed`.
    pub status: String,
    /// Lines added.
    pub lines_added: u64,
    /// Lines removed.
    pub lines_removed: u64,
}

impl From<&DiffStat> for FileRow {
    fn from(d: &DiffStat) -> Self {
        Self {
            path: d.path(),
            status: d.status.clone(),
            lines_added: d.lines_added,
            lines_removed: d.lines_removed,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct FileList(pub Vec<FileRow>);

impl Render for FileList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No changed files.".to_string();
        }
        const COLUMNS: [Column; 4] = [
            Column::keep("STATUS"),
            Column::keep("+"),
            Column::keep("-"),
            Column::flex("PATH"),
        ];
        let rows = self
            .0
            .iter()
            .map(|f| {
                vec![
                    f.status.clone(),
                    f.lines_added.to_string(),
                    f.lines_removed.to_string(),
                    f.path.clone(),
                ]
            })
            .collect();
        table::render(&COLUMNS, rows)
    }
}

// ------------------------------------------------------------ payload builders

pub(crate) fn create_payload(
    title: &str,
    description: Option<&str>,
    source: &str,
    destination: &str,
    draft: bool,
    close_source_branch: bool,
    reviewers: &[Account],
) -> Value {
    let mut body = json!({
        "title": title,
        "source": { "branch": { "name": source } },
        "destination": { "branch": { "name": destination } },
        "reviewers": reviewers.iter().map(|r| json!({ "uuid": r.uuid })).collect::<Vec<_>>(),
    });
    if let Some(description) = description {
        body["description"] = json!(description);
    }
    if draft {
        body["draft"] = json!(true);
    }
    if close_source_branch {
        body["close_source_branch"] = json!(true);
    }
    body
}

pub(crate) struct EditFields<'a> {
    pub title: Option<&'a str>,
    pub description: Option<&'a str>,
    pub destination: Option<&'a str>,
    pub draft: bool,
    pub ready: bool,
    pub close_source_branch: bool,
    pub keep_source_branch: bool,
}

/// Only fields the user gave are sent, so nothing else is overwritten.
pub(crate) fn edit_payload(fields: &EditFields) -> Result<Value> {
    let mut body = Map::new();
    if let Some(title) = fields.title {
        body.insert("title".into(), json!(title));
    }
    if let Some(description) = fields.description {
        body.insert("description".into(), json!(description));
    }
    if let Some(destination) = fields.destination {
        body.insert(
            "destination".into(),
            json!({ "branch": { "name": destination } }),
        );
    }
    if fields.draft {
        body.insert("draft".into(), json!(true));
    }
    if fields.ready {
        body.insert("draft".into(), json!(false));
    }
    if fields.close_source_branch {
        body.insert("close_source_branch".into(), json!(true));
    }
    if fields.keep_source_branch {
        body.insert("close_source_branch".into(), json!(false));
    }
    if body.is_empty() {
        return Err(BibuError::Usage(
            "nothing to change; pass at least one of --title, --description, --destination, \
             --draft, --ready, --close-source-branch, --keep-source-branch"
                .to_string(),
        ));
    }
    Ok(Value::Object(body))
}

pub(crate) fn merge_payload(
    strategy: Option<MergeStrategy>,
    message: Option<&str>,
    close_source_branch: bool,
    keep_source_branch: bool,
) -> Value {
    let mut body = Map::new();
    if let Some(strategy) = strategy {
        body.insert("merge_strategy".into(), json!(strategy.api_name()));
    }
    if let Some(message) = message {
        body.insert("message".into(), json!(message));
    }
    if close_source_branch {
        body.insert("close_source_branch".into(), json!(true));
    }
    if keep_source_branch {
        body.insert("close_source_branch".into(), json!(false));
    }
    Value::Object(body)
}

fn matches_author(pr: &PullRequest, wanted: &str, me: Option<&Account>) -> bool {
    let Some(author) = &pr.author else {
        return false;
    };
    if wanted.eq_ignore_ascii_case("me") {
        return me.is_some_and(|me| !me.uuid.is_empty() && me.uuid == author.uuid);
    }
    author.uuid == wanted
        || author.account_id.as_deref() == Some(wanted)
        || author
            .nickname
            .as_deref()
            .is_some_and(|n| n.eq_ignore_ascii_case(wanted))
        || author.display_name.eq_ignore_ascii_case(wanted)
}

fn default_title(source: &str, destination: &str) -> String {
    format!("Merge {source} into {destination}")
}

// -------------------------------------------------------------------- handlers

fn describe(pr: &PullRequest) -> String {
    format!(
        "#{} \"{}\" ({} -> {})",
        pr.id,
        pr.title,
        pr.source.branch_name(),
        pr.destination.branch_name()
    )
}

fn action(pull_request: u64, action: &'static str) -> ActionResult {
    ActionResult {
        pull_request,
        action,
    }
}

pub fn run(
    command: &PrCommand,
    cli: &Cli,
    ctx: &Context,
    term: &dyn Terminal,
    mode: Mode,
) -> Result<String> {
    let repo = repo::resolve(cli.repo.as_deref())?;
    let client = ctx.client()?;
    let yes = cli.yes;

    match command {
        PrCommand::List {
            state,
            author,
            source,
            destination,
            reviews,
            paging,
        } => {
            let me = match author.as_deref() {
                Some(a) if a.eq_ignore_ascii_case("me") => Some(user::current(&client)?),
                _ => None,
            };
            let prs = api::list(&client, &repo, state.api_name(), paging.limit(), |pr| {
                author
                    .as_deref()
                    .is_none_or(|a| matches_author(pr, a, me.as_ref()))
                    && source
                        .as_deref()
                        .is_none_or(|b| pr.source.branch_name() == b)
                    && destination
                        .as_deref()
                        .is_none_or(|b| pr.destination.branch_name() == b)
            })?;
            let mut rows = Vec::with_capacity(prs.len());
            for pr in &prs {
                rows.push(if *reviews {
                    PrSummary::new(&api::get(&client, &repo, pr.id)?, true)
                } else {
                    PrSummary::new(pr, false)
                });
            }
            Ok(render(&PrList(rows), mode))
        }
        PrCommand::View { id } => Ok(render(
            &PrDetail::new(&api::get(&client, &repo, *id)?),
            mode,
        )),
        PrCommand::Create {
            destination,
            source,
            title,
            description,
            draft,
            close_source_branch,
            no_default_reviewers,
        } => {
            let source = match source {
                Some(source) => source.clone(),
                None => repo::current_branch().ok_or_else(|| {
                    BibuError::Usage(
                        "cannot tell the source branch; pass --source or run inside a git \
                         checkout with a branch checked out"
                            .to_string(),
                    )
                })?,
            };
            let reviewers = if *no_default_reviewers {
                vec![]
            } else {
                api::default_reviewers(&client, &repo)?
            };

            let mut created: Vec<PrSummary> = Vec::new();
            for destination in destination {
                let title = title
                    .clone()
                    .unwrap_or_else(|| default_title(&source, destination));
                let body = create_payload(
                    &title,
                    description.as_deref(),
                    &source,
                    destination,
                    *draft,
                    *close_source_branch,
                    &reviewers,
                );
                match api::create(&client, &repo, &body) {
                    Ok(pr) => created.push(PrSummary::new(&pr, false)),
                    Err(err) if created.is_empty() => return Err(err),
                    Err(err) => {
                        return Err(with_context(
                            err,
                            &format!(
                                "already created: {}; failed on destination {destination}",
                                created
                                    .iter()
                                    .map(|p| format!("#{}", p.id))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        ))
                    }
                }
            }
            Ok(render(
                &Created {
                    pull_requests: created,
                },
                mode,
            ))
        }
        PrCommand::Edit {
            id,
            title,
            description,
            destination,
            draft,
            ready,
            close_source_branch,
            keep_source_branch,
        } => {
            let body = edit_payload(&EditFields {
                title: title.as_deref(),
                description: description.as_deref(),
                destination: destination.as_deref(),
                draft: *draft,
                ready: *ready,
                close_source_branch: *close_source_branch,
                keep_source_branch: *keep_source_branch,
            })?;
            let pr = api::update(&client, &repo, *id, &body)?;
            Ok(render(&PrDetail::new(&pr), mode))
        }
        PrCommand::Diff { id } => {
            let diff = api::diff(&client, &repo, *id)?;
            let result = DiffResult {
                pull_request: *id,
                diff,
            };
            // Raw text by default so `bibu pr diff 5 | less` works even though stdout is piped.
            Ok(render(
                &result,
                if cli.json { Mode::Json } else { Mode::Table },
            ))
        }
        PrCommand::Commits { id, paging } => {
            let commits = api::commits(&client, &repo, *id, paging.limit())?;
            Ok(render(
                &CommitList(commits.iter().map(CommitRow::from).collect()),
                mode,
            ))
        }
        PrCommand::Files { id, paging } => {
            let files = api::diffstat(&client, &repo, *id, paging.limit())?;
            Ok(render(
                &FileList(files.iter().map(FileRow::from).collect()),
                mode,
            ))
        }
        PrCommand::Approve { id } => {
            api::approve(&client, &repo, *id)?;
            Ok(render(&action(*id, "approved"), mode))
        }
        PrCommand::Unapprove { id } => {
            api::unapprove(&client, &repo, *id)?;
            Ok(render(&action(*id, "approval_removed"), mode))
        }
        PrCommand::RequestChanges { id } => {
            api::request_changes(&client, &repo, *id)?;
            Ok(render(&action(*id, "changes_requested"), mode))
        }
        PrCommand::UnrequestChanges { id } => {
            api::unrequest_changes(&client, &repo, *id)?;
            Ok(render(&action(*id, "change_request_removed"), mode))
        }
        PrCommand::Merge {
            id,
            strategy,
            message,
            close_source_branch,
            keep_source_branch,
        } => {
            confirm(term, yes, "merge", || {
                Ok(format!(
                    "Merge {}?",
                    describe(&api::get(&client, &repo, *id)?)
                ))
            })?;
            let body = merge_payload(
                *strategy,
                message.as_deref(),
                *close_source_branch,
                *keep_source_branch,
            );
            let pr = api::merge(&client, &repo, *id, &body)?;
            Ok(render(&PrDetail::new(&pr), mode))
        }
        PrCommand::Comment { command } => {
            super::comment::run(command, &repo, &client, term, yes, mode)
        }
        PrCommand::Reviewers { command } => super::reviewers::run(command, &repo, &client, mode),
        PrCommand::Task { command } => super::task::run(command, &repo, &client, term, mode),
        PrCommand::Decline { id } => {
            confirm(term, yes, "decline", || {
                Ok(format!(
                    "Decline {}?",
                    describe(&api::get(&client, &repo, *id)?)
                ))
            })?;
            let pr = api::decline(&client, &repo, *id)?;
            Ok(render(&PrDetail::new(&pr), mode))
        }
    }
}

/// Prefixes extra context onto an error while keeping its class (and so its exit code).
fn with_context(err: BibuError, context: &str) -> BibuError {
    let message = format!("{err} ({context})");
    match err {
        BibuError::Usage(_) => BibuError::Usage(message),
        BibuError::Auth(_) => BibuError::Auth(message),
        BibuError::Forbidden(_) => BibuError::Forbidden(message),
        BibuError::NotFound(_) => BibuError::NotFound(message),
        BibuError::Conflict(_) => BibuError::Conflict(message),
        BibuError::Network(_) => BibuError::Network(message),
        BibuError::Other(_) => BibuError::Other(message),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::testutil::{FakeTerminal, PIPED, TTY_SAYS_NO, TTY_SAYS_YES};

    fn account(name: &str, uuid: &str) -> Account {
        Account {
            display_name: name.into(),
            uuid: uuid.into(),
            nickname: Some(name.to_lowercase().replace(' ', "")),
            account_id: Some(format!("acct-{uuid}")),
        }
    }

    fn pr(json: Value) -> PullRequest {
        serde_json::from_value(json).unwrap()
    }

    // ---- payloads

    #[test]
    fn create_payload_minimal_has_only_required_keys_and_reviewer_uuids() {
        let body = create_payload(
            "T",
            None,
            "feat",
            "main",
            false,
            false,
            &[account("A B", "{1}")],
        );
        assert_eq!(
            body,
            json!({
                "title": "T",
                "source": {"branch": {"name": "feat"}},
                "destination": {"branch": {"name": "main"}},
                "reviewers": [{"uuid": "{1}"}],
            })
        );
    }

    #[test]
    fn create_payload_includes_optional_fields_only_when_set() {
        let body = create_payload("T", Some("desc"), "f", "m", true, true, &[]);
        assert_eq!(body["description"], "desc");
        assert_eq!(body["draft"], true);
        assert_eq!(body["close_source_branch"], true);
        assert_eq!(body["reviewers"], json!([]));
    }

    fn fields() -> EditFields<'static> {
        EditFields {
            title: None,
            description: None,
            destination: None,
            draft: false,
            ready: false,
            close_source_branch: false,
            keep_source_branch: false,
        }
    }

    #[test]
    fn edit_with_nothing_to_change_is_a_usage_error() {
        assert!(matches!(edit_payload(&fields()), Err(BibuError::Usage(_))));
    }

    #[test]
    fn edit_sends_only_the_given_fields() {
        let body = edit_payload(&EditFields {
            title: Some("New"),
            ..fields()
        })
        .unwrap();
        assert_eq!(body, json!({"title": "New"}));

        let body = edit_payload(&EditFields {
            description: Some(""),
            destination: Some("develop"),
            ..fields()
        })
        .unwrap();
        assert_eq!(
            body,
            json!({"description": "", "destination": {"branch": {"name": "develop"}}})
        );
    }

    #[test]
    fn edit_draft_ready_and_branch_cleanup_map_to_booleans() {
        assert_eq!(
            edit_payload(&EditFields {
                draft: true,
                ..fields()
            })
            .unwrap(),
            json!({"draft": true})
        );
        assert_eq!(
            edit_payload(&EditFields {
                ready: true,
                ..fields()
            })
            .unwrap(),
            json!({"draft": false})
        );
        assert_eq!(
            edit_payload(&EditFields {
                close_source_branch: true,
                ..fields()
            })
            .unwrap(),
            json!({"close_source_branch": true})
        );
        assert_eq!(
            edit_payload(&EditFields {
                keep_source_branch: true,
                ..fields()
            })
            .unwrap(),
            json!({"close_source_branch": false})
        );
    }

    #[test]
    fn merge_payload_defaults_to_an_empty_object() {
        assert_eq!(merge_payload(None, None, false, false), json!({}));
    }

    #[test]
    fn merge_payload_carries_strategy_message_and_cleanup() {
        assert_eq!(
            merge_payload(
                Some(MergeStrategy::SquashFastForward),
                Some("msg"),
                true,
                false
            ),
            json!({"merge_strategy": "squash_fast_forward", "message": "msg", "close_source_branch": true})
        );
        assert_eq!(
            merge_payload(None, None, false, true),
            json!({"close_source_branch": false})
        );
    }

    #[test]
    fn every_merge_strategy_uses_the_api_spelling() {
        use clap::ValueEnum;
        for strategy in MergeStrategy::value_variants() {
            let cli_name = strategy.to_possible_value().unwrap().get_name().to_string();
            assert_eq!(cli_name, strategy.api_name());
        }
    }

    // ---- filters & defaults

    #[test]
    fn author_filter_matches_nickname_name_uuid_and_account_id_case_insensitively() {
        let jane = account("Jane Doe", "{j}");
        let p = pr(json!({"id": 1, "author": jane}));
        for wanted in ["janedoe", "JANEDOE", "jane doe", "{j}", "acct-{j}"] {
            assert!(matches_author(&p, wanted, None), "{wanted}");
        }
        assert!(!matches_author(&p, "bob", None));
    }

    #[test]
    fn author_me_compares_uuids_and_needs_a_known_user() {
        let jane = account("Jane Doe", "{j}");
        let p = pr(json!({"id": 1, "author": jane.clone()}));
        assert!(matches_author(&p, "me", Some(&jane)));
        assert!(matches_author(&p, "ME", Some(&jane)));
        assert!(!matches_author(&p, "me", Some(&account("Bob", "{b}"))));
        assert!(!matches_author(&p, "me", None));
    }

    #[test]
    fn a_pr_without_author_never_matches() {
        assert!(!matches_author(&pr(json!({"id": 1})), "anyone", None));
    }

    #[test]
    fn default_title_names_both_branches() {
        assert_eq!(default_title("feat/x", "main"), "Merge feat/x into main");
    }

    // ---- confirmation

    #[test]
    fn yes_skips_the_prompt_entirely() {
        let asked = Cell::new(false);
        let result = confirm(&PIPED, true, "merge", || {
            asked.set(true);
            Ok(String::new())
        });
        assert!(result.is_ok());
        assert!(
            !asked.get(),
            "no question (and so no API call) when --yes is given"
        );
    }

    #[test]
    fn without_a_terminal_or_yes_the_action_is_refused() {
        let err = confirm(&PIPED, false, "merge", || Ok(String::new())).unwrap_err();
        assert!(
            matches!(&err, BibuError::Usage(m) if m.contains("--yes")),
            "{err:?}"
        );
    }

    #[test]
    fn a_terminal_user_can_accept_or_decline() {
        assert!(confirm(&TTY_SAYS_YES, false, "merge", || Ok("ok?".into())).is_ok());
        let err = confirm(&TTY_SAYS_NO, false, "merge", || Ok("ok?".into())).unwrap_err();
        assert!(matches!(err, BibuError::Other(m) if m.contains("aborted")));
    }

    #[test]
    fn confirm_accepts_yes_spellings_only() {
        for (answer, expected) in [
            ("y", true),
            ("YES", true),
            (" yes ", true),
            ("n", false),
            ("", false),
            ("yep", false),
        ] {
            let term = FakeTerminal {
                interactive: true,
                line: answer,
                secret: "",
                stdin: "",
            };
            assert_eq!(
                confirm(&term, false, "merge", || Ok("q".into())).is_ok(),
                expected,
                "{answer:?}"
            );
        }
    }

    #[test]
    fn with_context_keeps_the_error_class() {
        let err = with_context(BibuError::Conflict("boom".into()), "extra");
        assert_eq!(err, BibuError::Conflict("boom (extra)".into()));
        assert_eq!(
            with_context(BibuError::Network("n".into()), "x").exit_code(),
            7
        );
    }

    // ---- parsing leniency

    #[test]
    fn a_bare_id_is_a_valid_pull_request() {
        let p = pr(json!({"id": 9}));
        assert_eq!(p.id, 9);
        assert_eq!(p.source.branch_name(), "");
        assert_eq!(p.url(), "");
        assert!(p.participants.is_empty());
    }

    #[test]
    fn description_falls_back_to_summary_raw() {
        assert_eq!(
            pr(json!({"id": 1, "description": "d", "summary": {"raw": "s"}})).description_text(),
            "d"
        );
        assert_eq!(
            pr(json!({"id": 1, "summary": {"raw": "s"}})).description_text(),
            "s"
        );
        assert_eq!(pr(json!({"id": 1})).description_text(), "");
    }

    #[test]
    fn null_branch_after_deletion_does_not_break_parsing() {
        let p = pr(
            json!({"id": 1, "source": {"branch": null}, "destination": {"branch": {"name": "main"}}}),
        );
        assert_eq!(p.source.branch_name(), "");
        assert_eq!(p.destination.branch_name(), "main");
    }

    // ---- rendering

    fn detail() -> PrDetail {
        let p = pr(json!({
            "id": 12, "title": "Add thing", "state": "OPEN", "draft": true,
            "description": "Does a thing.\n",
            "author": account("Jane Doe", "{j}"),
            "source": {"branch": {"name": "feat/x"}}, "destination": {"branch": {"name": "main"}},
            "comment_count": 3, "task_count": 1,
            "created_on": "2026-09-01T10:00:00+00:00", "updated_on": "2026-09-02T11:00:00+00:00",
            "links": {"html": {"href": "https://bitbucket.org/acme/api/pull-requests/12"}},
            "participants": [
                {"user": account("Bob Ray", "{b}"), "role": "REVIEWER", "approved": true, "state": "approved"},
                {"user": account("Cy Lee", "{c}"), "role": "REVIEWER", "approved": false, "state": "changes_requested"},
                {"user": account("Di Fox", "{d}"), "role": "REVIEWER", "approved": false, "state": null},
                {"user": account("Ed Poe", "{e}"), "role": "PARTICIPANT", "approved": false, "state": null},
            ],
        }));
        PrDetail::new(&p)
    }

    #[test]
    fn detail_table_shows_everything_a_reviewer_needs() {
        let text = detail().render_table();
        for needle in [
            "#12 Add thing",
            "OPEN, draft | feat/x -> main",
            "Author: Jane Doe | created 2026-09-01 | updated 2026-09-02",
            "Comments: 3 | open tasks: 1",
            "https://bitbucket.org/acme/api/pull-requests/12",
            "Bob Ray - approved",
            "Cy Lee - changes requested",
            "Di Fox - pending",
            "Ed Poe - commented",
            "Does a thing.",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
    }

    #[test]
    fn detail_json_flattens_the_summary() {
        let v = serde_json::to_value(detail()).unwrap();
        assert_eq!(v["id"], 12);
        assert_eq!(v["source_branch"], "feat/x");
        assert_eq!(v["description"], "Does a thing.\n");
        assert_eq!(v["participants"][0]["user"]["display_name"], "Bob Ray");
        assert_eq!(v["participants"][1]["state"], "changes_requested");
        assert!(v["merge_commit"].is_null());
    }

    #[test]
    fn summary_omits_participants_unless_asked() {
        let p = pr(json!({"id": 1}));
        let v = serde_json::to_value(PrSummary::new(&p, false)).unwrap();
        assert!(v.get("participants").is_none());
        let v = serde_json::to_value(PrSummary::new(&p, true)).unwrap();
        assert_eq!(v["participants"], json!([]));
    }

    #[test]
    fn list_table_has_columns_and_a_reviews_column_only_when_loaded() {
        let p = pr(json!({
            "id": 7, "title": "Fix", "state": "OPEN", "author": account("Jane Doe", "{j}"),
            "source": {"branch": {"name": "a"}}, "destination": {"branch": {"name": "b"}},
            "updated_on": "2026-09-02T11:00:00+00:00"
        }));
        let plain = PrList(vec![PrSummary::new(&p, false)]).render_table();
        assert!(plain.contains("#7") && plain.contains("Fix") && plain.contains("a -> b"));
        assert!(plain.contains("Jane Doe") && plain.contains("2026-09-02"));
        assert!(!plain.contains("REVIEWS"));

        let with = pr(json!({"id": 7, "participants": [
            {"user": account("Bob Ray", "{b}"), "role": "REVIEWER", "approved": true, "state": "approved"}
        ]}));
        let reviewed = PrList(vec![PrSummary::new(&with, true)]).render_table();
        assert!(reviewed.contains("REVIEWS") && reviewed.contains("Bob Ray (approved)"));
    }

    #[test]
    fn list_table_marks_drafts_and_handles_empty() {
        let p = pr(json!({"id": 7, "title": "WIP", "draft": true}));
        assert!(PrList(vec![PrSummary::new(&p, false)])
            .render_table()
            .contains("[draft] WIP"));
        assert_eq!(PrList(vec![]).render_table(), "No pull requests found.");
    }

    #[test]
    fn list_json_is_a_bare_array() {
        let v = serde_json::to_value(PrList(vec![])).unwrap();
        assert_eq!(v, json!([]));
    }

    #[test]
    fn diff_table_is_the_raw_diff() {
        let d = DiffResult {
            pull_request: 1,
            diff: "diff --git a b\n+x\n".into(),
        };
        assert_eq!(d.render_table(), "diff --git a b\n+x");
    }

    #[test]
    fn commit_and_file_tables() {
        let commit: Commit = serde_json::from_value(json!({
            "hash": "abcdef1234567", "date": "2026-09-03T00:00:00+00:00",
            "message": "Subject line\n\nbody",
            "author": {"raw": "Jane Doe <j@x.io>"}
        }))
        .unwrap();
        let row = CommitRow::from(&commit);
        assert_eq!(
            (row.author.as_str(), row.subject.as_str()),
            ("Jane Doe", "Subject line")
        );
        let text = CommitList(vec![row]).render_table();
        assert!(text.contains("abcdef1") && !text.contains("abcdef12"));

        let stat: DiffStat = serde_json::from_value(json!({
            "status": "removed", "lines_added": 0, "lines_removed": 4,
            "old": {"path": "gone.rs"}, "new": null
        }))
        .unwrap();
        let row = FileRow::from(&stat);
        assert_eq!(row.path, "gone.rs");
        let text = FileList(vec![row]).render_table();
        assert!(text.contains("removed") && text.contains("gone.rs"));
    }

    #[test]
    fn commit_author_prefers_the_linked_account() {
        let commit: Commit = serde_json::from_value(json!({
            "author": {"raw": "jdoe <j@x.io>", "user": account("Jane Doe", "{j}")}
        }))
        .unwrap();
        assert_eq!(commit.author.name(), "Jane Doe");
    }

    #[test]
    fn action_result_reads_naturally() {
        assert_eq!(
            action(5, "change_request_removed").render_table(),
            "PR #5: change request removed"
        );
    }
}
