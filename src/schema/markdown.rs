//! Generated reference docs: `docs/commands.md` and `docs/json-output.md`.
//!
//! Both are rendered from the same tree `bibu schema` prints. `tests/docs.rs` fails when the
//! committed files differ from these functions; run `UPDATE_DOCS=1 cargo test --test docs`.

use serde_json::Value;

use super::{error_schema, leaves, tree, ArgInfo, CommandInfo};
use crate::error::EXIT_CODES;

fn anchor(path: &[String]) -> String {
    format!("bibu-{}", path.join("-"))
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn flag(arg: &ArgInfo) -> String {
    if arg.positional {
        let name = arg
            .value_name
            .clone()
            .unwrap_or_else(|| arg.name.to_uppercase());
        return format!("`<{name}>`");
    }
    let mut out = String::new();
    if let Some(short) = arg.short {
        out.push_str(&format!("`-{short}`, "));
    }
    if let Some(long) = &arg.long {
        out.push_str(&format!("`--{long}"));
        if arg.takes_value {
            out.push_str(&format!(
                " <{}>",
                arg.value_name
                    .clone()
                    .unwrap_or_else(|| arg.name.to_uppercase())
            ));
        }
        out.push('`');
    }
    out
}

fn arg_notes(arg: &ArgInfo) -> String {
    let mut notes = Vec::new();
    if arg.required {
        notes.push("required".to_string());
    }
    if arg.repeatable {
        notes.push("repeatable".to_string());
    }
    if !arg.possible_values.is_empty() {
        let names: Vec<_> = arg
            .possible_values
            .iter()
            .map(|v| format!("`{}`", v.name))
            .collect();
        notes.push(format!("one of {}", names.join(", ")));
    }
    if let Some(default) = &arg.default {
        notes.push(format!("default `{default}`"));
    }
    if let Some(env) = &arg.env {
        notes.push(format!("env `{env}`"));
    }
    notes.join("; ")
}

fn args_table(args: &[ArgInfo]) -> String {
    let mut out = String::from("| Argument | Description | Notes |\n|---|---|---|\n");
    for arg in args {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            flag(arg),
            cell(arg.help.as_deref().unwrap_or("")),
            cell(&arg_notes(arg))
        ));
    }
    out
}

// ------------------------------------------------------------ schema -> field table

fn nullable_parts(schema: &Value) -> (Value, bool) {
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let non_null: Vec<&Value> = variants
            .iter()
            .filter(|v| v.get("type") != Some(&Value::String("null".into())))
            .collect();
        if non_null.len() == 1 && non_null.len() < variants.len() {
            let mut inner = non_null[0].clone();
            if let (Some(desc), Some(obj)) = (schema.get("description"), inner.as_object_mut()) {
                obj.entry("description").or_insert_with(|| desc.clone());
            }
            return (inner, true);
        }
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let non_null: Vec<&Value> = types
            .iter()
            .filter(|t| t.as_str() != Some("null"))
            .collect();
        if non_null.len() == 1 && non_null.len() < types.len() {
            let mut inner = schema.clone();
            inner["type"] = non_null[0].clone();
            return (inner, true);
        }
    }
    (schema.clone(), false)
}

fn type_name(schema: &Value) -> String {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .filter_map(Value::as_str)
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(" \\| ");
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("array") => format!("array of {}", type_name(&schema["items"])),
        Some(t) => t.to_string(),
        None => "any".to_string(),
    }
}

/// Rows of (field path, type, description) for every property, recursively.
pub(crate) fn field_rows(schema: &Value, prefix: &str, rows: &mut Vec<(String, String, String)>) {
    let (schema, _) = nullable_parts(schema);
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        let (items, _) = nullable_parts(&schema["items"]);
        if items.get("properties").is_some() {
            field_rows(&items, &format!("{prefix}[]"), rows);
        }
        return;
    }
    let Some(props) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (name, raw) in props {
        let (prop, nullable) = nullable_parts(raw);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        let mut ty = type_name(&prop);
        if nullable {
            ty.push_str(" or null");
        }
        let desc = prop
            .get("description")
            .or_else(|| raw.get("description"))
            .and_then(Value::as_str)
            .unwrap_or("");
        rows.push((path.clone(), ty, desc.to_string()));
        if prop.get("type").and_then(Value::as_str) == Some("object")
            || prop.get("type").and_then(Value::as_str) == Some("array")
        {
            field_rows(&prop, &path, rows);
        }
    }
}

fn fields_table(schema: &Value) -> String {
    let (top, _) = nullable_parts(schema);
    let root_is_array = top.get("type").and_then(Value::as_str) == Some("array");
    let mut rows = Vec::new();
    field_rows(&top, "", &mut rows);
    let mut out = String::new();
    if root_is_array {
        out.push_str("The value is an array; each element has:\n\n");
    }
    out.push_str("| Field | Type | Description |\n|---|---|---|\n");
    for (path, ty, desc) in rows {
        let path = if root_is_array {
            format!("[].{path}")
        } else {
            path
        };
        out.push_str(&format!("| `{path}` | {} | {} |\n", cell(&ty), cell(&desc)));
    }
    out
}

// ------------------------------------------------------------------ documents

fn command_section(cmd: &CommandInfo) -> String {
    let title = format!("bibu {}", cmd.path.join(" "));
    let mut out = format!("## `{title}`\n\n");
    if let Some(about) = &cmd.about {
        out.push_str(&format!("{about}\n\n"));
    }
    if let Some(long) = &cmd.long_about {
        if Some(long) != cmd.about.as_ref() {
            out.push_str(&format!("```text\n{}\n```\n\n", long.trim_end()));
        }
    }
    out.push_str(&format!("```text\n{}\n```\n\n", cmd.usage.trim_end()));
    if !cmd.aliases.is_empty() {
        out.push_str(&format!(
            "Aliases: {}\n\n",
            cmd.aliases
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !cmd.args.is_empty() {
        out.push_str(&args_table(&cmd.args));
        out.push('\n');
    }
    if let Some(output) = &cmd.output {
        out.push_str(&format!("**Output** ({}", output.description));
        if output.raw_text_by_default {
            out.push_str("; raw text unless `--json` is given");
        }
        out.push_str(")\n\n");
        out.push_str(&fields_table(&output.schema));
        out.push('\n');
    }
    out
}

/// `docs/commands.md`
pub fn commands() -> String {
    let root = tree();
    let mut out = String::from(
        "# bibu command reference\n\n\
         > Generated from the CLI definition. Do not edit by hand: change the code, then run\n\
         > `UPDATE_DOCS=1 cargo test --test docs`. CI fails when this file is out of date.\n\
         > The same information is available as JSON from `bibu schema`.\n\n\
         ## Global options\n\nThese are accepted by every command.\n\n",
    );
    out.push_str(&args_table(&root.args));
    out.push_str("\n## Commands\n\n");
    for leaf in leaves(&root) {
        out.push_str(&format!(
            "- [`bibu {}`](#{})",
            leaf.path.join(" "),
            anchor(&leaf.path)
        ));
        if let Some(about) = &leaf.about {
            out.push_str(&format!(": {about}"));
        }
        out.push('\n');
    }
    out.push('\n');
    for leaf in leaves(&root) {
        out.push_str(&command_section(leaf));
    }
    out.trim_end().to_string() + "\n"
}

/// `docs/json-output.md`
pub fn json_output() -> String {
    let mut out = String::from(
        "# Output, errors and exit codes\n\n\
         > Generated from the CLI definition. Do not edit by hand: run\n\
         > `UPDATE_DOCS=1 cargo test --test docs`.\n\n\
         ## Output modes\n\n\
         - In a terminal, commands print a human-readable table or text.\n\
         - When stdout is **not** a terminal (piped, captured by a script or an agent), or when `--json`\n\
           is passed, commands print JSON. Pass `--json` explicitly when you depend on it.\n\
         - Results go to **stdout**; errors and prompts go to **stderr**. Nothing is written to stdout\n\
           when a command fails.\n\
         - `pr diff` and `pipeline logs` print raw text by default, even when piped, so they work with\n\
           `grep` and `less`; add `--json` to get them wrapped in JSON.\n\
         - `bibu schema` always prints JSON.\n\
         - bibu never asks a question without a terminal. Destructive commands (`pr merge`,\n\
           `pr decline`, `pr comment delete`, `branch delete`) need `--yes` in that case and exit 2\n\
           without it.\n\n\
         ## Exit codes\n\n| Code | `error.code` | Meaning |\n|---|---|---|\n",
    );
    for e in EXIT_CODES {
        out.push_str(&format!(
            "| {} | `{}` | {} |\n",
            e.code,
            e.name,
            cell(e.meaning)
        ));
    }
    out.push_str("\n## Error format\n\nOn failure, stderr gets one line of JSON (or `error: ...` text in a terminal):\n\n```json\n{\"error\":{\"code\":\"not_found\",\"message\":\"no current reviewer matches \\\"bob\\\"\",\"hint\":null}}\n```\n\n");
    out.push_str(&fields_table(&error_schema()));
    out.push_str("\nBranch on the exit code or `error.code`; never parse `error.message`.\n\n## Credentials\n\nSet `BIBU_EMAIL` and `BIBU_TOKEN` (both) to authenticate without the keychain, which is what scripts and agents should do. `BIBU_REPO` (or `--repo`) selects the repository, `BIBU_API_BASE` points at another API root (tests).\n");
    out
}
