//! `bibu pr comment list | add | reply | edit | delete | resolve | reopen`

use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Map, Value};

use super::{confirm, read_text};
use crate::api::models::comment::{Comment, Inline};
use crate::api::models::Account;
use crate::api::{comments as api, Client};
use crate::cli::CommentCommand;
use crate::error::{BibuError, Result};
use crate::output::{render, Mode, Render};
use crate::repo::RepoRef;
use crate::terminal::Terminal;

// ---------------------------------------------------------------- output types

/// Where a comment sits. `side` is `new`, `old`, or `file` for a whole-file comment.
#[derive(Debug, Serialize, PartialEq, Eq, JsonSchema)]
pub struct InlineRow {
    /// File path, relative to the repository root.
    pub path: String,
    /// First line (the only line for a single-line comment); null for a whole-file comment.
    pub line: Option<u64>,
    /// Last line when the comment spans a range.
    pub end_line: Option<u64>,
    /// Which version of the file the lines refer to: `new`, `old`, or `file` for a whole-file comment.
    pub side: &'static str,
}

impl From<&Inline> for InlineRow {
    fn from(i: &Inline) -> Self {
        if i.to.is_some() {
            Self {
                path: i.path.clone(),
                line: i.start_to.or(i.to),
                end_line: i.start_to.and(i.to),
                side: "new",
            }
        } else if i.from.is_some() {
            Self {
                path: i.path.clone(),
                line: i.start_from.or(i.from),
                end_line: i.start_from.and(i.from),
                side: "old",
            }
        } else {
            Self {
                path: i.path.clone(),
                line: None,
                end_line: None,
                side: "file",
            }
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentRow {
    /// Comment number.
    pub id: u64,
    /// Number of the comment this replies to; null for the start of a thread.
    pub parent_id: Option<u64>,
    /// Who wrote the comment.
    pub author: Option<Account>,
    /// ISO 8601 creation time.
    pub created_on: String,
    /// ISO 8601 time of the last edit.
    pub updated_on: String,
    /// Comment text (markdown).
    pub content: String,
    /// Where the comment is anchored; null for a general comment.
    pub inline: Option<InlineRow>,
    /// Whether the thread is resolved (recorded on the thread's first comment).
    pub resolved: bool,
    /// Who resolved the thread, when Bitbucket reports it.
    pub resolved_by: Option<Account>,
    /// Deleted comments are hidden from `list` unless `--include-deleted`.
    pub deleted: bool,
    /// A draft comment of an unfinished review.
    pub pending: bool,
    /// Link to the comment in Bitbucket.
    pub url: String,
}

impl From<&Comment> for CommentRow {
    fn from(c: &Comment) -> Self {
        Self {
            id: c.id,
            parent_id: c.parent.as_ref().map(|p| p.id),
            author: c.user.clone(),
            created_on: c.created_on.clone(),
            updated_on: c.updated_on.clone(),
            content: c.content.raw.clone(),
            inline: c.inline.as_ref().map(InlineRow::from),
            resolved: c.resolution.is_some(),
            resolved_by: c.resolution.as_ref().and_then(|r| r.user.clone()),
            deleted: c.deleted,
            pending: c.pending,
            url: c.url(),
        }
    }
}

fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

impl CommentRow {
    fn header(&self) -> String {
        let author = self
            .author
            .as_ref()
            .map_or("unknown", |a| a.display_name.as_str());
        let mut head = format!("#{} {} {}", self.id, author, day(&self.created_on));
        // A reply lives in its parent's thread; repeating the anchor is noise.
        if let (Some(inline), None) = (&self.inline, self.parent_id) {
            head.push(' ');
            head.push_str(&inline.path);
            match (inline.line, inline.end_line) {
                (Some(start), Some(end)) => head.push_str(&format!(":{start}-{end}")),
                (Some(line), None) => head.push_str(&format!(":{line}")),
                _ => {}
            }
            if inline.side == "old" {
                head.push_str(" (old)");
            }
        }
        if self.resolved {
            match &self.resolved_by {
                Some(by) if !by.display_name.is_empty() => {
                    head.push_str(&format!(" [resolved by {}]", by.display_name));
                }
                _ => head.push_str(" [resolved]"),
            }
        }
        if self.pending {
            head.push_str(" [pending]");
        }
        if self.deleted {
            head.push_str(" [deleted]");
        }
        head
    }

    fn block(&self, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let mut lines = vec![format!("{pad}{}", self.header())];
        lines.extend(self.content.lines().map(|l| format!("{pad}  {l}")));
        lines.join("\n")
    }
}

impl Render for CommentRow {
    fn render_table(&self) -> String {
        format!("{}\n{}", self.block(0), self.url)
            .trim_end()
            .to_string()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct CommentList(pub Vec<CommentRow>);

/// Depth-first thread order: each root, then its replies, oldest first. A reply whose parent is
/// not in the list (filtered out or missing) is shown as a root.
pub(crate) fn thread_order(rows: &[CommentRow]) -> Vec<(usize, &CommentRow)> {
    let ids: HashSet<u64> = rows.iter().map(|r| r.id).collect();
    let mut children: HashMap<u64, Vec<&CommentRow>> = HashMap::new();
    let mut roots: Vec<&CommentRow> = Vec::new();
    for row in rows {
        match row.parent_id.filter(|p| ids.contains(p)) {
            Some(parent) => children.entry(parent).or_default().push(row),
            None => roots.push(row),
        }
    }
    let mut out = Vec::with_capacity(rows.len());
    let mut seen = HashSet::new();
    let mut stack: Vec<(usize, &CommentRow)> = roots.into_iter().rev().map(|r| (0, r)).collect();
    while let Some((depth, row)) = stack.pop() {
        if !seen.insert(row.id) {
            continue;
        }
        out.push((depth, row));
        if let Some(replies) = children.get(&row.id) {
            stack.extend(replies.iter().rev().map(|r| (depth + 1, *r)));
        }
    }
    out
}

impl Render for CommentList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No comments.".to_string();
        }
        thread_order(&self.0)
            .into_iter()
            .map(|(depth, row)| row.block(depth * 4))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentAction {
    /// Pull request number.
    pub pull_request: u64,
    /// Comment number.
    pub comment: u64,
    /// What happened: `deleted`, `resolved` or `reopened`.
    pub action: &'static str,
}

impl Render for CommentAction {
    fn render_table(&self) -> String {
        format!(
            "Comment #{} on PR #{}: {}",
            self.comment, self.pull_request, self.action
        )
    }
}

// ------------------------------------------------------------ filters & payloads

#[derive(Debug, Default)]
pub(crate) struct Filter {
    pub unresolved: bool,
    pub inline: bool,
    pub file: Option<String>,
    pub include_deleted: bool,
}

/// Applies thread-level filters: a reply is kept or dropped together with the comment that
/// started its thread, so a resolved thread never leaves orphaned replies behind.
pub(crate) fn filter_comments(comments: Vec<Comment>, filter: &Filter) -> Vec<Comment> {
    let by_id: HashMap<u64, &Comment> = comments.iter().map(|c| (c.id, c)).collect();
    let root_of = |c: &Comment| -> u64 {
        let mut current = c;
        for _ in 0..1000 {
            match current.parent.as_ref().and_then(|p| by_id.get(&p.id)) {
                Some(parent) => current = parent,
                None => break,
            }
        }
        current.id
    };
    let keep: HashSet<u64> = comments
        .iter()
        .filter(|c| filter.include_deleted || !c.deleted)
        .filter(|c| {
            let root = by_id[&root_of(c)];
            (!filter.unresolved || root.resolution.is_none())
                && (!(filter.inline || filter.file.is_some()) || root.inline.is_some())
                && filter
                    .file
                    .as_deref()
                    .is_none_or(|f| root.inline.as_ref().is_some_and(|i| i.path == f))
        })
        .map(|c| c.id)
        .collect();
    comments
        .into_iter()
        .filter(|c| keep.contains(&c.id))
        .collect()
}

/// A file, and optionally a line or range within it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Anchor {
    pub path: String,
    pub line: Option<u64>,
    pub end_line: Option<u64>,
    pub old_side: bool,
}

impl Anchor {
    pub(crate) fn new(
        file: &str,
        line: Option<u64>,
        end_line: Option<u64>,
        old_side: bool,
    ) -> Result<Self> {
        let mut path = file.trim();
        while let Some(rest) = path.strip_prefix("./") {
            path = rest;
        }
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return Err(BibuError::Usage(
                "--file must be a path inside the repository".into(),
            ));
        }
        if let (Some(start), Some(end)) = (line, end_line) {
            if end < start {
                return Err(BibuError::Usage(format!(
                    "--end-line ({end}) must not be before --line ({start})"
                )));
            }
        }
        Ok(Self {
            path: path.to_string(),
            line,
            end_line,
            old_side,
        })
    }
}

/// Body for `POST .../comments`. Anchors and replies are mutually exclusive by construction:
/// a reply lives in its parent's thread and cannot carry its own position.
pub(crate) fn comment_payload(text: &str, anchor: Option<&Anchor>, parent: Option<u64>) -> Value {
    let mut body = json!({ "content": { "raw": text } });
    if let Some(anchor) = anchor {
        let mut inline = Map::new();
        inline.insert("path".into(), json!(anchor.path));
        if let Some(line) = anchor.line {
            let (start_key, end_key) = if anchor.old_side {
                ("start_from", "from")
            } else {
                ("start_to", "to")
            };
            match anchor.end_line {
                Some(end) => {
                    inline.insert(start_key.into(), json!(line));
                    inline.insert(end_key.into(), json!(end));
                }
                None => {
                    inline.insert(end_key.into(), json!(line));
                }
            }
        }
        body["inline"] = Value::Object(inline);
    }
    if let Some(parent) = parent {
        body["parent"] = json!({ "id": parent });
    }
    body
}

// -------------------------------------------------------------------- handlers

fn action(pull_request: u64, comment: u64, action: &'static str) -> CommentAction {
    CommentAction {
        pull_request,
        comment,
        action,
    }
}

pub fn run(
    command: &CommentCommand,
    repo: &RepoRef,
    client: &Client,
    term: &dyn Terminal,
    yes: bool,
    mode: Mode,
) -> Result<String> {
    match command {
        CommentCommand::List {
            id,
            unresolved,
            inline,
            file,
            include_deleted,
            paging,
        } => {
            let all = api::list(client, repo, *id, paging.limit())?;
            let filter = Filter {
                unresolved: *unresolved,
                inline: *inline,
                file: file.clone(),
                include_deleted: *include_deleted,
            };
            let rows = filter_comments(all, &filter)
                .iter()
                .map(CommentRow::from)
                .collect();
            Ok(render(&CommentList(rows), mode))
        }
        CommentCommand::Add {
            id,
            text,
            file,
            line,
            end_line,
            old_side,
        } => {
            let text = read_text(term, text.as_deref())?;
            let anchor = match file {
                Some(file) => Some(Anchor::new(file, *line, *end_line, *old_side)?),
                None => None,
            };
            let created = api::create(
                client,
                repo,
                *id,
                &comment_payload(&text, anchor.as_ref(), None),
            )?;
            Ok(render(&CommentRow::from(&created), mode))
        }
        CommentCommand::Reply {
            id,
            comment_id,
            text,
        } => {
            let text = read_text(term, text.as_deref())?;
            let created = api::create(
                client,
                repo,
                *id,
                &comment_payload(&text, None, Some(*comment_id)),
            )?;
            Ok(render(&CommentRow::from(&created), mode))
        }
        CommentCommand::Edit {
            id,
            comment_id,
            text,
        } => {
            let text = read_text(term, text.as_deref())?;
            let updated = api::update(
                client,
                repo,
                *id,
                *comment_id,
                &json!({ "content": { "raw": text } }),
            )?;
            Ok(render(&CommentRow::from(&updated), mode))
        }
        CommentCommand::Delete { id, comment_id } => {
            confirm(term, yes, "delete the comment", || {
                Ok(format!("Delete comment #{comment_id} on PR #{id}?"))
            })?;
            api::delete(client, repo, *id, *comment_id)?;
            Ok(render(&action(*id, *comment_id, "deleted"), mode))
        }
        CommentCommand::Resolve { id, comment_id } => {
            api::resolve(client, repo, *id, *comment_id)?;
            Ok(render(&action(*id, *comment_id, "resolved"), mode))
        }
        CommentCommand::Reopen { id, comment_id } => {
            api::reopen(client, repo, *id, *comment_id)?;
            Ok(render(&action(*id, *comment_id, "reopened"), mode))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(json: Value) -> Comment {
        serde_json::from_value(json).unwrap()
    }

    fn row(id: u64, parent: Option<u64>, text: &str) -> CommentRow {
        CommentRow::from(&comment(json!({
            "id": id, "content": {"raw": text}, "created_on": "2026-09-30T10:00:00+00:00",
            "user": {"display_name": "Jane Doe", "uuid": "{j}"},
            "parent": parent.map(|p| json!({"id": p})),
        })))
    }

    // ---- payloads

    #[test]
    fn general_comment_has_only_content() {
        assert_eq!(
            comment_payload("hi", None, None),
            json!({"content": {"raw": "hi"}})
        );
    }

    #[test]
    fn file_comment_has_only_a_path() {
        let anchor = Anchor::new("src/app.py", None, None, false).unwrap();
        assert_eq!(
            comment_payload("hi", Some(&anchor), None),
            json!({"content": {"raw": "hi"}, "inline": {"path": "src/app.py"}})
        );
    }

    #[test]
    fn single_line_uses_to_for_the_new_side_and_from_for_the_old() {
        let new = Anchor::new("a.rs", Some(7), None, false).unwrap();
        assert_eq!(
            comment_payload("x", Some(&new), None)["inline"],
            json!({"path": "a.rs", "to": 7})
        );
        let old = Anchor::new("a.rs", Some(7), None, true).unwrap();
        assert_eq!(
            comment_payload("x", Some(&old), None)["inline"],
            json!({"path": "a.rs", "from": 7})
        );
    }

    #[test]
    fn ranges_use_start_and_end_keys_for_their_side() {
        let new = Anchor::new("a.rs", Some(4), Some(6), false).unwrap();
        assert_eq!(
            comment_payload("x", Some(&new), None)["inline"],
            json!({"path": "a.rs", "start_to": 4, "to": 6})
        );
        let old = Anchor::new("a.rs", Some(4), Some(6), true).unwrap();
        assert_eq!(
            comment_payload("x", Some(&old), None)["inline"],
            json!({"path": "a.rs", "start_from": 4, "from": 6})
        );
    }

    #[test]
    fn a_reply_carries_its_parent_and_no_position() {
        let body = comment_payload("re", None, Some(42));
        assert_eq!(
            body,
            json!({"content": {"raw": "re"}, "parent": {"id": 42}})
        );
    }

    #[test]
    fn anchor_normalises_the_path() {
        for input in [
            "src/a.rs",
            "./src/a.rs",
            "/src/a.rs",
            "././src/a.rs",
            "  src/a.rs ",
        ] {
            assert_eq!(
                Anchor::new(input, None, None, false).unwrap().path,
                "src/a.rs",
                "{input}"
            );
        }
    }

    #[test]
    fn anchor_rejects_empty_paths_and_backwards_ranges() {
        for bad in ["", "  ", "./", "/"] {
            assert!(
                matches!(
                    Anchor::new(bad, None, None, false),
                    Err(BibuError::Usage(_))
                ),
                "{bad:?}"
            );
        }
        assert!(Anchor::new("a", Some(5), Some(4), false).is_err());
        assert!(Anchor::new("a", Some(5), Some(5), false).is_ok());
    }

    // ---- inline rows

    #[test]
    fn inline_row_reports_side_line_and_range() {
        let inline = |v: Value| InlineRow::from(&serde_json::from_value::<Inline>(v).unwrap());
        assert_eq!(
            inline(json!({"path": "a", "to": 3})),
            InlineRow {
                path: "a".into(),
                line: Some(3),
                end_line: None,
                side: "new"
            }
        );
        assert_eq!(
            inline(json!({"path": "a", "start_to": 4, "to": 6})),
            InlineRow {
                path: "a".into(),
                line: Some(4),
                end_line: Some(6),
                side: "new"
            }
        );
        assert_eq!(
            inline(json!({"path": "a", "from": 2})),
            InlineRow {
                path: "a".into(),
                line: Some(2),
                end_line: None,
                side: "old"
            }
        );
        assert_eq!(
            inline(json!({"path": "a"})),
            InlineRow {
                path: "a".into(),
                line: None,
                end_line: None,
                side: "file"
            }
        );
    }

    // ---- filters

    fn thread() -> Vec<Comment> {
        vec![
            comment(json!({"id": 1, "content": {"raw": "general"}})),
            comment(
                json!({"id": 2, "content": {"raw": "inline open"}, "inline": {"path": "a.rs", "to": 1}}),
            ),
            comment(json!({"id": 3, "content": {"raw": "reply to 2"}, "parent": {"id": 2}})),
            comment(
                json!({"id": 4, "content": {"raw": "inline resolved"}, "inline": {"path": "b.rs", "to": 2}, "resolution": {}}),
            ),
            comment(json!({"id": 5, "content": {"raw": "reply to 4"}, "parent": {"id": 4}})),
            comment(
                json!({"id": 6, "content": {"raw": "gone"}, "deleted": true, "inline": {"path": "a.rs", "to": 9}}),
            ),
        ]
    }

    fn ids(comments: &[Comment]) -> Vec<u64> {
        comments.iter().map(|c| c.id).collect()
    }

    #[test]
    fn default_filter_only_hides_deleted_comments() {
        assert_eq!(
            ids(&filter_comments(thread(), &Filter::default())),
            [1, 2, 3, 4, 5]
        );
        let all = Filter {
            include_deleted: true,
            ..Filter::default()
        };
        assert_eq!(ids(&filter_comments(thread(), &all)), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn unresolved_drops_resolved_threads_together_with_their_replies() {
        let f = Filter {
            unresolved: true,
            ..Filter::default()
        };
        assert_eq!(ids(&filter_comments(thread(), &f)), [1, 2, 3]);
    }

    #[test]
    fn inline_keeps_only_anchored_threads() {
        let f = Filter {
            inline: true,
            ..Filter::default()
        };
        assert_eq!(ids(&filter_comments(thread(), &f)), [2, 3, 4, 5]);
    }

    #[test]
    fn file_filter_matches_the_thread_root_path_and_implies_inline() {
        let f = Filter {
            file: Some("a.rs".into()),
            ..Filter::default()
        };
        assert_eq!(ids(&filter_comments(thread(), &f)), [2, 3]);
        let none = Filter {
            file: Some("nope.rs".into()),
            ..Filter::default()
        };
        assert!(filter_comments(thread(), &none).is_empty());
    }

    #[test]
    fn filters_combine() {
        let f = Filter {
            unresolved: true,
            inline: true,
            ..Filter::default()
        };
        assert_eq!(ids(&filter_comments(thread(), &f)), [2, 3]);
    }

    #[test]
    fn replies_to_a_deleted_root_follow_the_roots_state() {
        let comments = vec![
            comment(json!({"id": 1, "deleted": true, "resolution": {}, "inline": {"path": "a"}})),
            comment(json!({"id": 2, "parent": {"id": 1}})),
        ];
        let f = Filter {
            unresolved: true,
            ..Filter::default()
        };
        assert!(filter_comments(comments, &f).is_empty());
    }

    // ---- ordering and rendering

    #[test]
    fn threads_are_grouped_depth_first_and_oldest_first() {
        let rows = vec![
            row(1, None, "a"),
            row(2, None, "b"),
            row(3, Some(1), "reply a"),
            row(4, Some(3), "nested"),
        ];
        let order: Vec<(usize, u64)> = thread_order(&rows)
            .iter()
            .map(|(d, r)| (*d, r.id))
            .collect();
        assert_eq!(order, [(0, 1), (1, 3), (2, 4), (0, 2)]);
    }

    #[test]
    fn orphaned_replies_are_shown_as_roots() {
        let rows = vec![row(5, Some(99), "orphan")];
        assert_eq!(thread_order(&rows)[0].0, 0);
    }

    #[test]
    fn a_parent_cycle_cannot_loop_forever() {
        let rows = vec![row(1, Some(2), "a"), row(2, Some(1), "b")];
        assert!(thread_order(&rows).len() <= 2);
    }

    #[test]
    fn table_shows_anchor_flags_and_indents_replies() {
        let mut rows = vec![
            CommentRow::from(&comment(json!({
                "id": 10, "content": {"raw": "line one\nline two"}, "created_on": "2026-09-30T10:00:00+00:00",
                "user": {"display_name": "Jane Doe", "uuid": "{j}"},
                "inline": {"path": "src/app.py", "start_to": 3, "to": 5},
                "resolution": {"user": {"display_name": "Bob Ray", "uuid": "{b}"}},
            }))),
            row(11, Some(10), "a reply"),
        ];
        rows.push(CommentRow::from(&comment(json!({
            "id": 12, "content": {"raw": "old line"}, "inline": {"path": "x.rs", "from": 7}, "pending": true
        }))));
        let text = CommentList(rows).render_table();
        assert!(
            text.contains("#10 Jane Doe 2026-09-30 src/app.py:3-5 [resolved by Bob Ray]"),
            "{text}"
        );
        assert!(text.contains("  line one\n  line two"), "{text}");
        assert!(
            text.contains("    #11 Jane Doe 2026-09-30\n      a reply"),
            "{text}"
        );
        assert!(text.contains("x.rs:7 (old) [pending]"), "{text}");
    }

    #[test]
    fn replies_do_not_repeat_the_parents_anchor_but_json_keeps_it() {
        let reply = CommentRow::from(&comment(json!({
            "id": 2, "parent": {"id": 1}, "content": {"raw": "r"},
            "inline": {"path": "src/app.py", "to": 3}
        })));
        assert!(!reply.header().contains("src/app.py"), "{}", reply.header());
        assert_eq!(
            serde_json::to_value(&reply).unwrap()["inline"]["path"],
            "src/app.py"
        );
    }

    #[test]
    fn empty_list_message_and_json_shape() {
        assert_eq!(CommentList(vec![]).render_table(), "No comments.");
        let v = serde_json::to_value(row(1, Some(2), "t")).unwrap();
        assert_eq!(v["parent_id"], 2);
        assert_eq!(v["content"], "t");
        assert_eq!(v["resolved"], false);
        assert!(v["inline"].is_null());
    }

    #[test]
    fn action_text() {
        assert_eq!(
            action(4, 9, "resolved").render_table(),
            "Comment #9 on PR #4: resolved"
        );
    }
}
