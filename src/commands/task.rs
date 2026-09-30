//! `bibu pr task list | add | resolve | reopen`

use comfy_table::presets::NOTHING;
use comfy_table::Table;
use serde::Serialize;
use serde_json::{json, Value};

use super::read_text;
use crate::api::models::task::Task;
use crate::api::models::Account;
use crate::api::{tasks as api, Client};
use crate::cli::TaskCommand;
use crate::error::Result;
use crate::output::{render, Mode, Render};
use crate::repo::RepoRef;
use crate::terminal::Terminal;

#[derive(Debug, Serialize)]
pub struct TaskRow {
    pub id: u64,
    /// `RESOLVED` or `UNRESOLVED`.
    pub state: String,
    pub content: String,
    pub creator: Option<Account>,
    pub created_on: String,
    pub resolved_by: Option<Account>,
    pub resolved_on: Option<String>,
    /// The comment this task is attached to, if any.
    pub comment_id: Option<u64>,
}

impl From<&Task> for TaskRow {
    fn from(t: &Task) -> Self {
        Self {
            id: t.id,
            state: t.state.clone(),
            content: t.content.raw.clone(),
            creator: t.creator.clone(),
            created_on: t.created_on.clone(),
            resolved_by: t.resolved_by.clone().filter(|a| !a.uuid.is_empty()),
            resolved_on: t.resolved_on.clone(),
            comment_id: t.comment.as_ref().map(|c| c.id),
        }
    }
}

fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

impl TaskRow {
    fn line(&self) -> String {
        let mark = if self.state == "RESOLVED" {
            "[x]"
        } else {
            "[ ]"
        };
        let by = self
            .creator
            .as_ref()
            .map_or("unknown", |c| c.display_name.as_str());
        let mut line = format!(
            "#{} {mark} {} ({by}, {})",
            self.id,
            self.content,
            day(&self.created_on)
        );
        if let Some(comment) = self.comment_id {
            line.push_str(&format!(" on comment #{comment}"));
        }
        line
    }
}

impl Render for TaskRow {
    fn render_table(&self) -> String {
        self.line()
    }
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct TaskList(pub Vec<TaskRow>);

impl Render for TaskList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No tasks.".to_string();
        }
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec!["ID", "DONE", "TASK", "BY", "CREATED", "COMMENT"]);
        for t in &self.0 {
            table.add_row(vec![
                format!("#{}", t.id),
                if t.state == "RESOLVED" { "yes" } else { "no" }.to_string(),
                t.content.clone(),
                t.creator
                    .as_ref()
                    .map(|c| c.display_name.clone())
                    .unwrap_or_default(),
                day(&t.created_on).to_string(),
                t.comment_id.map(|c| format!("#{c}")).unwrap_or_default(),
            ]);
        }
        table.to_string()
    }
}

pub(crate) fn create_payload(text: &str, comment: Option<u64>) -> Value {
    let mut body = json!({ "content": { "raw": text } });
    if let Some(comment) = comment {
        body["comment"] = json!({ "id": comment });
    }
    body
}

pub(crate) fn state_payload(resolved: bool) -> Value {
    json!({ "state": if resolved { "RESOLVED" } else { "UNRESOLVED" } })
}

pub fn run(
    command: &TaskCommand,
    repo: &RepoRef,
    client: &Client,
    term: &dyn Terminal,
    mode: Mode,
) -> Result<String> {
    match command {
        TaskCommand::List {
            id,
            unresolved,
            paging,
        } => {
            let tasks = api::list(client, repo, *id, paging.limit())?;
            let rows = tasks
                .iter()
                .filter(|t| !*unresolved || t.state != "RESOLVED")
                .map(TaskRow::from)
                .collect();
            Ok(render(&TaskList(rows), mode))
        }
        TaskCommand::Add { id, text, comment } => {
            let text = read_text(term, text.as_deref())?;
            let task = api::create(client, repo, *id, &create_payload(&text, *comment))?;
            Ok(render(&TaskRow::from(&task), mode))
        }
        TaskCommand::Resolve { id, task_id } => {
            let task = api::update(client, repo, *id, *task_id, &state_payload(true))?;
            Ok(render(&TaskRow::from(&task), mode))
        }
        TaskCommand::Reopen { id, task_id } => {
            let task = api::update(client, repo, *id, *task_id, &state_payload(false))?;
            Ok(render(&TaskRow::from(&task), mode))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(json: Value) -> Task {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn create_payload_attaches_a_comment_only_when_given() {
        assert_eq!(
            create_payload("do it", None),
            json!({"content": {"raw": "do it"}})
        );
        assert_eq!(
            create_payload("do it", Some(7)),
            json!({"content": {"raw": "do it"}, "comment": {"id": 7}})
        );
    }

    #[test]
    fn state_payload_sends_state_only() {
        assert_eq!(state_payload(true), json!({"state": "RESOLVED"}));
        assert_eq!(state_payload(false), json!({"state": "UNRESOLVED"}));
    }

    #[test]
    fn row_maps_fields_and_drops_an_empty_resolver() {
        let t = task(json!({
            "id": 5, "state": "UNRESOLVED", "content": {"raw": "fix"},
            "creator": {"display_name": "Jane", "uuid": "{j}"},
            "created_on": "2026-09-30T10:00:00+00:00",
            "resolved_by": null, "comment": {"id": 9}
        }));
        let row = TaskRow::from(&t);
        assert_eq!((row.id, row.comment_id), (5, Some(9)));
        assert!(row.resolved_by.is_none());
        let v = serde_json::to_value(&row).unwrap();
        assert_eq!(v["content"], "fix");
        assert_eq!(v["state"], "UNRESOLVED");
    }

    #[test]
    fn single_line_shows_checkbox_author_and_comment() {
        let open = TaskRow::from(&task(json!({
            "id": 5, "state": "UNRESOLVED", "content": {"raw": "fix"},
            "creator": {"display_name": "Jane", "uuid": "{j}"}, "created_on": "2026-09-30T10:00:00+00:00",
            "comment": {"id": 9}
        })));
        assert_eq!(
            open.render_table(),
            "#5 [ ] fix (Jane, 2026-09-30) on comment #9"
        );
        let done = TaskRow::from(&task(
            json!({"id": 6, "state": "RESOLVED", "content": {"raw": "ok"}}),
        ));
        assert!(done.render_table().starts_with("#6 [x] ok (unknown, "));
    }

    #[test]
    fn list_table_and_empty_message() {
        assert_eq!(TaskList(vec![]).render_table(), "No tasks.");
        let text = TaskList(vec![TaskRow::from(&task(json!({
            "id": 5, "state": "RESOLVED", "content": {"raw": "fix"},
            "creator": {"display_name": "Jane", "uuid": "{j}"}, "comment": {"id": 9}
        })))])
        .render_table();
        assert!(
            text.contains("#5")
                && text.contains("yes")
                && text.contains("fix")
                && text.contains("#9")
        );
    }
}
