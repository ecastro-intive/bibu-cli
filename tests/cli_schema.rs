//! End-to-end tests for `bibu schema`: it must work with no login, no repository and no network.

use assert_cmd::Command;
use serde_json::Value;

fn bibu() -> Command {
    let mut cmd = Command::cargo_bin("bibu").unwrap();
    cmd.env_remove("BIBU_EMAIL")
        .env_remove("BIBU_TOKEN")
        .env_remove("BIBU_REPO")
        .env("BIBU_API_BASE", "http://127.0.0.1:1")
        .env(
            "BIBU_CREDENTIALS_FILE",
            std::env::temp_dir().join("bibu-no-such-credentials.json"),
        )
        .current_dir(std::env::temp_dir());
    cmd
}

fn run(args: &[&str]) -> (Value, std::process::Output) {
    let out = bibu().args(args).output().unwrap();
    let value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    (value, out)
}

#[test]
fn works_without_credentials_repository_or_network() {
    let (doc, out) = run(&["schema"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    assert_eq!(doc["name"], "bibu");
    assert_eq!(doc["schema_version"], 1);
    assert!(doc["exit_codes"].as_array().unwrap().len() >= 8);
    assert!(doc["command"]["subcommands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["name"] == "pr"));
}

#[test]
fn the_default_index_is_compact_and_full_adds_every_schema() {
    let (compact, _) = run(&["schema"]);
    let (full, _) = run(&["schema", "--full"]);
    let compact_size = compact["command"].to_string().len();
    let full_size = full["command"].to_string().len();
    assert!(!compact["command"].to_string().contains("\"properties\""));
    assert!(
        full_size > compact_size * 2,
        "{compact_size} vs {full_size}"
    );
}

#[test]
fn naming_a_command_returns_its_arguments_and_output_schema() {
    let (doc, out) = run(&["schema", "pr", "comment", "add"]);
    assert!(out.status.success());
    let command = &doc["command"];
    assert_eq!(command["path"], serde_json::json!(["pr", "comment", "add"]));
    let names: Vec<&str> = command["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    for expected in ["id", "text", "file", "line", "end_line", "old_side"] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
    assert_eq!(command["output"]["format"], "json");
    assert!(command["output"]["schema"]["properties"]["inline"].is_object());
}

#[test]
fn aliases_resolve_to_the_canonical_command() {
    let (doc, out) = run(&["schema", "pr", "no-approve"]);
    assert!(out.status.success());
    assert_eq!(doc["command"]["name"], "unapprove");
}

#[test]
fn an_unknown_command_is_not_found_and_lists_the_choices() {
    let (_, out) = run(&["schema", "pr", "frobnicate"]);
    assert_eq!(out.status.code(), Some(5));
    assert!(out.stdout.is_empty());
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["error"]["code"], "not_found");
    assert!(err["error"]["message"]
        .as_str()
        .unwrap()
        .contains("comment"));
}

#[test]
fn it_is_listed_in_help() {
    let out = bibu().arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("schema"));
}

#[test]
fn piped_output_is_minified_to_save_tokens() {
    let out = bibu().args(["schema", "pr", "list"]).output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        text.trim_end().lines().count(),
        1,
        "one line of JSON when piped"
    );
    assert!(
        text.len() < 12_000,
        "a single command's document should stay small, got {}",
        text.len()
    );
}

#[test]
fn absent_values_are_omitted_rather_than_null() {
    let (doc, _) = run(&["schema", "pr", "approve"]);
    let id = &doc["command"]["args"][0];
    assert_eq!(id["name"], "id");
    assert!(id.get("long").is_none() && id.get("short").is_none() && id.get("env").is_none());
    assert_eq!(id["positional"], true);
}
