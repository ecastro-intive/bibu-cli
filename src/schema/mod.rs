//! `bibu schema`: a machine-readable description of the whole CLI, for agents and docs.
//!
//! The tree is read from the clap definition, so it is always in step with the real arguments.
//! Output schemas come from [`outputs`]; the generated docs come from [`markdown`].

pub mod markdown;
pub mod outputs;

use clap::{Arg, ArgAction, Command, CommandFactory};
use serde::Serialize;
use serde_json::{json, Value};

use crate::cli::Cli;
use crate::error::{BibuError, Result, EXIT_CODES};
use outputs::OutputInfo;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct PossibleValue {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArgInfo {
    /// Flag name without dashes, or the positional's name.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    pub positional: bool,
    pub required: bool,
    /// `false` for plain switches such as `--yes`.
    pub takes_value: bool,
    /// May be given more than once or take several values.
    pub repeatable: bool,
    /// Declared on the root command and accepted by every subcommand.
    pub global: bool,
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub possible_values: Vec<PossibleValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandInfo {
    pub name: String,
    /// Full path from the root, e.g. `["pr", "comment", "add"]`.
    pub path: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long_about: Option<String>,
    pub usage: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<ArgInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<CommandInfo>,
    /// Present on commands that print something (leaf commands).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputInfo>,
}

fn styled(text: Option<&clap::builder::StyledStr>) -> Option<String> {
    text.map(|t| t.to_string()).filter(|t| !t.trim().is_empty())
}

fn arg_info(arg: &Arg) -> ArgInfo {
    let action = arg.get_action();
    let takes_value = action.takes_values();
    let repeatable = matches!(action, ArgAction::Append)
        || arg.get_num_args().is_some_and(|n| n.max_values() > 1);
    ArgInfo {
        name: arg.get_id().to_string(),
        long: arg.get_long().map(str::to_string),
        short: arg.get_short(),
        positional: arg.is_positional(),
        required: arg.is_required_set(),
        takes_value,
        repeatable,
        global: arg.is_global_set(),
        help: styled(arg.get_long_help().or(arg.get_help())),
        value_name: arg
            .get_value_names()
            .and_then(|names| names.first())
            .map(|n| n.to_string())
            .filter(|_| takes_value),
        possible_values: arg
            .get_possible_values()
            .iter()
            .filter(|v| !v.is_hide_set())
            .map(|v| PossibleValue {
                name: v.get_name().to_string(),
                help: v.get_help().map(|h| h.to_string()),
            })
            .collect(),
        default: arg
            .get_default_values()
            .first()
            .map(|v| v.to_string_lossy().into_owned()),
        env: arg.get_env().map(|e| e.to_string_lossy().into_owned()),
    }
}

fn describe(cmd: &Command, path: Vec<String>, is_root: bool) -> CommandInfo {
    let args = cmd
        .get_arguments()
        .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
        // A global argument is described once, on the root.
        .filter(|a| is_root || !a.is_global_set())
        .map(arg_info)
        .collect();
    let subcommands: Vec<CommandInfo> = cmd
        .get_subcommands()
        .filter(|c| c.get_name() != "help")
        .map(|c| {
            let mut child_path = path.clone();
            child_path.push(c.get_name().to_string());
            describe(c, child_path, false)
        })
        .collect();
    let output = if subcommands.is_empty() {
        let segments: Vec<&str> = path.iter().map(String::as_str).collect();
        outputs::for_path(&segments)
    } else {
        None
    };
    CommandInfo {
        name: if is_root {
            "bibu".to_string()
        } else {
            cmd.get_name().to_string()
        },
        aliases: cmd.get_all_aliases().map(str::to_string).collect(),
        about: styled(cmd.get_about()),
        long_about: styled(cmd.get_long_about()),
        usage: cmd
            .clone()
            .render_usage()
            .to_string()
            .trim_start_matches("Usage: ")
            .to_string(),
        path,
        args,
        subcommands,
        output,
    }
}

/// The full command tree of the CLI.
pub fn tree() -> CommandInfo {
    let mut root = Cli::command();
    root.build();
    describe(&root, Vec::new(), true)
}

/// The command at `path`, matching subcommand names and aliases.
pub fn find<'a>(root: &'a CommandInfo, path: &[String]) -> Result<&'a CommandInfo> {
    let mut current = root;
    for (depth, wanted) in path.iter().enumerate() {
        current = current
            .subcommands
            .iter()
            .find(|c| c.name == *wanted || c.aliases.iter().any(|a| a == wanted))
            .ok_or_else(|| {
                let valid = current
                    .subcommands
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>();
                let after = if depth == 0 {
                    "the commands are".to_string()
                } else {
                    format!("after `{}` the choices are", path[..depth].join(" "))
                };
                BibuError::NotFound(format!(
                    "no command {wanted:?}; {after}: {}",
                    valid.join(", ")
                ))
            })?;
    }
    Ok(current)
}

/// Every leaf (runnable) command, depth first.
pub fn leaves(node: &CommandInfo) -> Vec<&CommandInfo> {
    if node.subcommands.is_empty() {
        vec![node]
    } else {
        node.subcommands.iter().flat_map(leaves).collect()
    }
}

/// JSON Schema of the error envelope printed on stderr.
pub fn error_schema() -> Value {
    outputs::schema_of::<crate::error::ErrorEnvelope>()
}

fn exit_codes() -> Value {
    Value::Array(
        EXIT_CODES
            .iter()
            .map(|e| json!({"code": e.code, "error_code": e.name, "meaning": e.meaning}))
            .collect(),
    )
}

/// A copy of `command` and its descendants with every output schema removed.
fn without_schemas(command: &CommandInfo) -> CommandInfo {
    let mut copy = command.clone();
    strip(&mut copy);
    copy
}

fn strip(command: &mut CommandInfo) {
    command.long_about = None;
    if let Some(output) = &mut command.output {
        output.schema = Value::Null;
    }
    command.subcommands.iter_mut().for_each(strip);
}

/// What `bibu schema [COMMAND...]` prints. Output schemas are included when the path names a
/// single runnable command or when `full` is set; otherwise the result is a compact index.
pub fn document(path: &[String], full: bool) -> Result<Value> {
    // `bibu schema "pr comment add"` (one quoted word) works like three words.
    let path: Vec<String> = path
        .iter()
        .flat_map(|p| p.split_whitespace())
        .map(str::to_string)
        .collect();
    let root = tree();
    let command = find(&root, &path)?;
    let detailed = full || command.subcommands.is_empty();
    let command = if detailed {
        command.clone()
    } else {
        without_schemas(command)
    };
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "name": "bibu",
        "version": env!("CARGO_PKG_VERSION"),
        "command": command,
        "exit_codes": exit_codes(),
        "error_output": error_schema(),
    }))
}

/// `pretty` is for people at a terminal; everyone else gets minified JSON, which costs far
/// fewer tokens when an agent reads it.
pub fn run(path: &[String], full: bool, pretty: bool) -> Result<String> {
    let document = document(path, full)?;
    Ok(if pretty {
        serde_json::to_string_pretty(&document)
    } else {
        serde_json::to_string(&document)
    }
    .expect("schema document serializes"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::commands::pipeline::{LogsResult, StepLog};
    use crate::commands::pr::{PrDetail, PrList};

    fn all(node: &CommandInfo) -> Vec<&CommandInfo> {
        let mut out = vec![node];
        for child in &node.subcommands {
            out.extend(all(child));
        }
        out
    }

    #[test]
    fn every_runnable_command_declares_its_output() {
        let root = tree();
        let missing: Vec<String> = leaves(&root)
            .into_iter()
            .filter(|c| c.output.is_none())
            .map(|c| c.path.join(" "))
            .collect();
        assert!(
            missing.is_empty(),
            "commands without an output schema: {missing:?}"
        );
    }

    #[test]
    fn every_command_and_argument_is_documented() {
        let root = tree();
        for cmd in all(&root).into_iter().filter(|c| !c.path.is_empty()) {
            assert!(
                cmd.about.is_some(),
                "`bibu {}` has no description",
                cmd.path.join(" ")
            );
            for arg in &cmd.args {
                assert!(
                    arg.help.is_some(),
                    "`bibu {}` argument `{}` has no help",
                    cmd.path.join(" "),
                    arg.name
                );
            }
        }
        for arg in &root.args {
            assert!(
                arg.help.is_some(),
                "global argument `{}` has no help",
                arg.name
            );
        }
    }

    #[test]
    fn every_output_field_is_documented() {
        let root = tree();
        let mut undocumented = Vec::new();
        for leaf in leaves(&root) {
            let mut rows = Vec::new();
            markdown::field_rows(&leaf.output.as_ref().unwrap().schema, "", &mut rows);
            assert!(
                !rows.is_empty() || leaf.path == ["repo"],
                "`bibu {}` has no output fields",
                leaf.path.join(" ")
            );
            for (field, _, description) in rows {
                if description.trim().is_empty() {
                    undocumented.push(format!("bibu {}: {field}", leaf.path.join(" ")));
                }
            }
        }
        assert!(
            undocumented.is_empty(),
            "output fields without a description: {undocumented:#?}"
        );
    }

    #[test]
    fn global_flags_are_described_once_on_the_root() {
        let root = tree();
        let globals: Vec<&str> = root
            .args
            .iter()
            .filter(|a| a.global)
            .map(|a| a.name.as_str())
            .collect();
        for expected in ["repo", "json", "yes"] {
            assert!(
                globals.contains(&expected),
                "{expected} missing from {globals:?}"
            );
        }
        for cmd in all(&root).into_iter().filter(|c| !c.path.is_empty()) {
            assert!(
                cmd.args.iter().all(|a| !a.global),
                "`{}` repeats a global",
                cmd.path.join(" ")
            );
        }
    }

    #[test]
    fn argument_details_are_accurate() {
        let root = tree();
        let add = find(&root, &["pr".into(), "comment".into(), "add".into()]).unwrap();
        let arg = |n: &str| {
            add.args
                .iter()
                .find(|a| a.name == n)
                .unwrap_or_else(|| panic!("no {n}"))
        };
        assert!(arg("id").positional && arg("id").required);
        assert!(arg("text").positional && !arg("text").required);
        assert_eq!(
            (arg("file").long.as_deref(), arg("file").short),
            (Some("file"), Some('f'))
        );
        assert!(arg("file").takes_value && !arg("old_side").takes_value);

        let create = find(&root, &["pr".into(), "create".into()]).unwrap();
        let destination = create
            .args
            .iter()
            .find(|a| a.name == "destination")
            .unwrap();
        assert!(destination.required && destination.repeatable);

        let list = find(&root, &["pr".into(), "list".into()]).unwrap();
        let state = list.args.iter().find(|a| a.name == "state").unwrap();
        assert_eq!(state.default.as_deref(), Some("open"));
        assert!(state.possible_values.iter().any(|v| v.name == "merged"));
        let limit = list.args.iter().find(|a| a.name == "limit").unwrap();
        assert_eq!(limit.default.as_deref(), Some("25"));
    }

    #[test]
    fn find_follows_names_and_aliases_and_explains_misses() {
        let root = tree();
        assert_eq!(
            find(&root, &["pr".into(), "no-approve".into()])
                .unwrap()
                .name,
            "unapprove"
        );
        assert!(find(&root, &[]).unwrap().path.is_empty());
        let err = find(&root, &["pr".into(), "nope".into()]).unwrap_err();
        assert!(
            matches!(&err, BibuError::NotFound(m) if m.contains("approve") && m.contains("after `pr`")),
            "{err:?}"
        );
    }

    #[test]
    fn a_quoted_path_works_like_separate_words() {
        let split = document(&["pr".into(), "comment".into(), "add".into()], false).unwrap();
        let quoted = document(&["pr comment add".into()], false).unwrap();
        assert_eq!(split, quoted);
    }

    #[test]
    fn the_miss_message_reads_naturally_at_every_depth() {
        let root = tree();
        let top = find(&root, &["nope".into()]).unwrap_err().to_string();
        assert!(
            top.starts_with("no command \"nope\"; the commands are: "),
            "{top}"
        );
        let nested = find(&root, &["pr".into(), "nope".into()])
            .unwrap_err()
            .to_string();
        assert!(nested.contains("after `pr` the choices are: "), "{nested}");
    }

    #[test]
    fn group_calls_are_compact_and_single_commands_are_detailed() {
        let compact = document(&["pr".into()], false).unwrap();
        let text = compact["command"].to_string();
        assert!(
            !text.contains("\"properties\""),
            "compact index must not carry output schemas"
        );
        assert!(text.contains("pr comment") || text.contains("\"comment\""));

        let detailed = document(&["pr".into(), "list".into()], false).unwrap();
        assert!(detailed["command"]["output"]["schema"]["type"] == "array");

        let full = document(&["pr".into()], true).unwrap();
        assert!(full["command"].to_string().contains("\"properties\""));
    }

    #[test]
    fn the_document_carries_version_exit_codes_and_the_error_shape() {
        let doc = document(&[], false).unwrap();
        assert_eq!(doc["schema_version"], SCHEMA_VERSION);
        assert_eq!(doc["name"], "bibu");
        assert_eq!(doc["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            doc["exit_codes"].as_array().unwrap().len(),
            EXIT_CODES.len()
        );
        assert_eq!(
            doc["error_output"]["properties"]["error"]["properties"]["code"]["type"],
            "string"
        );
    }

    #[test]
    fn raw_text_commands_are_flagged() {
        let root = tree();
        for (path, raw) in [
            (["pr", "diff"], true),
            (["pipeline", "logs"], true),
            (["pr", "list"], false),
        ] {
            let cmd = find(&root, &path.map(String::from)).unwrap();
            assert_eq!(
                cmd.output.as_ref().unwrap().raw_text_by_default,
                raw,
                "{path:?}"
            );
        }
    }

    // ---- schemas describe what commands really print

    fn assert_valid<T: schemars::JsonSchema>(instance: &Value) {
        let schema = outputs::schema_of::<T>();
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");
        let errors: Vec<String> = validator
            .iter_errors(instance)
            .map(|e| format!("{e} at {}", e.instance_path()))
            .collect();
        assert!(
            errors.is_empty(),
            "instance does not match its schema: {errors:?}\n{instance}"
        );
    }

    fn pr_json() -> Value {
        json!({
            "id": 1, "title": "t", "state": "OPEN", "description": "d", "draft": false,
            "author": {"display_name": "Jane", "uuid": "{j}", "nickname": "jane", "account_id": "1:a"},
            "source": {"branch": {"name": "a"}}, "destination": {"branch": {"name": "b"}},
            "participants": [{"user": {"display_name": "Bob", "uuid": "{b}"}, "role": "REVIEWER", "approved": false, "state": null}],
            "created_on": "2026-09-30T10:00:00+00:00", "updated_on": "2026-09-30T10:00:00+00:00"
        })
    }

    #[test]
    fn flattened_and_nullable_fields_validate() {
        let pr: crate::api::models::pullrequest::PullRequest =
            serde_json::from_value(pr_json()).unwrap();
        let detail = serde_json::to_value(PrDetail::new(&pr)).unwrap();
        assert_valid::<PrDetail>(&detail);
        // flatten: summary fields sit at the top level next to the extra ones
        assert!(detail["id"].is_number() && detail["description"].is_string());
        let schema = outputs::schema_of::<PrDetail>();
        assert!(
            schema["properties"]["id"].is_object()
                && schema["properties"]["merge_commit"].is_object()
        );
    }

    #[test]
    fn transparent_lists_are_arrays() {
        let pr: crate::api::models::pullrequest::PullRequest =
            serde_json::from_value(pr_json()).unwrap();
        let list = serde_json::to_value(PrList(vec![crate::commands::pr::PrSummary::new(
            &pr, false,
        )]))
        .unwrap();
        assert_valid::<PrList>(&list);
        assert_eq!(outputs::schema_of::<PrList>()["type"], "array");
        assert_valid::<PrList>(&json!([]));
        assert!(jsonschema::validator_for(&outputs::schema_of::<PrList>())
            .unwrap()
            .validate(&json!({"not": "an array"}))
            .is_err());
    }

    #[test]
    fn logs_result_with_and_without_a_log_validates() {
        let value = serde_json::to_value(LogsResult {
            pipeline: 3,
            steps: vec![
                StepLog {
                    uuid: "{s}".into(),
                    name: "build".into(),
                    status: "failed".into(),
                    log: Some("x".into()),
                },
                StepLog {
                    uuid: "{t}".into(),
                    name: "test".into(),
                    status: "pending".into(),
                    log: None,
                },
            ],
        })
        .unwrap();
        assert_valid::<LogsResult>(&value);
    }

    #[test]
    fn the_error_envelope_validates_with_and_without_a_hint() {
        for error in [BibuError::Auth("x".into()), BibuError::NotFound("y".into())] {
            assert_valid::<crate::error::ErrorEnvelope>(&error.to_json());
        }
    }

    #[test]
    fn every_output_schema_is_a_valid_json_schema() {
        let root = tree();
        for leaf in leaves(&root) {
            let output = leaf.output.as_ref().unwrap();
            assert!(
                jsonschema::validator_for(&output.schema).is_ok(),
                "`bibu {}` has an invalid schema",
                leaf.path.join(" ")
            );
        }
    }
}
