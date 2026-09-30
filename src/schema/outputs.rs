//! Which output type each command prints, as JSON Schema.
//!
//! The schemas come from the same structs that produce the output (`#[derive(JsonSchema)]`),
//! so they cannot drift from what commands really print.

use schemars::generate::SchemaSettings;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{json, Value};

use crate::commands::auth::{LoginResult, LogoutResult, StatusResult};
use crate::commands::branch::{BranchAction, BranchList, BranchRow};
use crate::commands::comment::{CommentAction, CommentList, CommentRow};
use crate::commands::member::MemberList;
use crate::commands::pipeline::{LogsResult, PipelineDetail, PipelineList};
use crate::commands::pr::{
    ActionResult, CommitList, Created, DiffResult, FileList, PrDetail, PrList,
};
use crate::commands::reviewers::ReviewerList;
use crate::commands::task::{TaskList, TaskRow};
use crate::commands::upgrade::UpgradeResult;
use crate::repo::RepoRef;

/// What a command prints on stdout.
#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    /// Always `json` for machine use.
    pub format: &'static str,
    /// One-line description of the top-level value.
    pub description: &'static str,
    /// JSON Schema (draft 2020-12) of the value printed with `--json` or when piped.
    #[serde(skip_serializing_if = "Value::is_null")]
    pub schema: Value,
    /// `true` when, without `--json`, stdout is raw text even if piped (`pr diff`,
    /// `pipeline logs`), so it can go through `grep` or `less`.
    pub raw_text_by_default: bool,
}

/// Schema of `T` with every sub-schema inlined, so readers never chase `$ref`s.
pub fn schema_of<T: JsonSchema>() -> Value {
    let generator = SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .into_generator();
    serde_json::to_value(generator.into_root_schema_for::<T>()).expect("schema serializes")
}

fn json_out<T: JsonSchema>(description: &'static str) -> OutputInfo {
    OutputInfo {
        format: "json",
        description,
        schema: schema_of::<T>(),
        raw_text_by_default: false,
    }
}

fn raw_out<T: JsonSchema>(description: &'static str) -> OutputInfo {
    OutputInfo {
        raw_text_by_default: true,
        ..json_out::<T>(description)
    }
}

/// Output of `bibu schema`, described by hand because the value is built from the very tree
/// being described.
fn schema_command_output() -> OutputInfo {
    OutputInfo {
        format: "json",
        description: "the command tree: commands, arguments, output schemas, exit codes",
        schema: json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "description": "With no argument the whole tree; with a command path, that command and its subcommands.",
            "properties": {
                "schema_version": {"type": "integer", "description": "Version of this document's layout; bumped on incompatible changes."},
                "name": {"type": "string", "description": "Always `bibu`."},
                "version": {"type": "string", "description": "Version of the bibu binary."},
                "command": {"type": "object", "description": "A command: name, path, aliases, about, usage, args, subcommands and output."},
                "exit_codes": {"type": "array", "items": {"type": "object"}, "description": "The exit-code contract: code, error_code and meaning."},
                "error_output": {"type": "object", "description": "JSON Schema of the error envelope printed on stderr."}
            }
        }),
        raw_text_by_default: false,
    }
}

/// The output description for a command path such as `["pr", "comment", "add"]`.
pub fn for_path(path: &[&str]) -> Option<OutputInfo> {
    Some(match path {
        ["auth", "login"] => {
            json_out::<LoginResult>("the verified account and where the login was stored")
        }
        ["auth", "status"] => json_out::<StatusResult>("the account behind the active credentials"),
        ["auth", "logout"] => json_out::<LogoutResult>("whether a stored login was removed"),
        ["repo"] => json_out::<RepoRef>("the resolved workspace and repository slug"),
        ["pr", "list"] => json_out::<PrList>("array of pull requests"),
        ["pr", "view" | "edit" | "merge" | "decline"] => {
            json_out::<PrDetail>("one pull request with description and participants")
        }
        ["pr", "create"] => json_out::<Created>("the pull requests that were created"),
        ["pr", "diff"] => raw_out::<DiffResult>("the unified diff (raw text by default)"),
        ["pr", "commits"] => json_out::<CommitList>("array of commits"),
        ["pr", "files"] => json_out::<FileList>("array of changed files"),
        ["pr", "approve" | "unapprove" | "request-changes" | "unrequest-changes"] => {
            json_out::<ActionResult>("the pull request and what happened")
        }
        ["pr", "comment", "list"] => json_out::<CommentList>("array of comments, oldest first"),
        ["pr", "comment", "add" | "reply" | "edit"] => json_out::<CommentRow>("the comment"),
        ["pr", "comment", "delete" | "resolve" | "reopen"] => {
            json_out::<CommentAction>("the comment and what happened")
        }
        ["pr", "reviewers", "list" | "add" | "remove"] => {
            json_out::<ReviewerList>("the pull request's reviewers and their review state")
        }
        ["pr", "task", "list"] => json_out::<TaskList>("array of tasks"),
        ["pr", "task", "add" | "resolve" | "reopen"] => json_out::<TaskRow>("the task"),
        ["pipeline", "list"] => json_out::<PipelineList>("array of pipeline runs, newest first"),
        ["pipeline", "view"] => json_out::<PipelineDetail>("one pipeline run with its steps"),
        ["pipeline", "logs"] => raw_out::<LogsResult>("the step logs (raw text by default)"),
        ["branch", "list"] => json_out::<BranchList>("array of branches"),
        ["branch", "create"] => json_out::<BranchRow>("the new branch"),
        ["branch", "delete"] => json_out::<BranchAction>("the branch and what happened"),
        ["member", "list" | "find"] => json_out::<MemberList>("array of workspace members"),
        ["upgrade"] => json_out::<UpgradeResult>(
            "the running and latest versions, and whether bibu was upgraded",
        ),
        ["schema"] => schema_command_output(),
        _ => return None,
    })
}
