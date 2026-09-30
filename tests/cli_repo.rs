//! End-to-end tests for `bibu repo` and the global output/exit-code contract.

use assert_cmd::Command;
use predicates::prelude::*;

fn bibu() -> Command {
    let mut cmd = Command::cargo_bin("bibu").unwrap();
    // Isolate from the developer's environment.
    cmd.env_remove("BIBU_REPO")
        .current_dir(std::env::temp_dir());
    cmd
}

#[test]
fn repo_from_flag_is_json_when_piped() {
    let out = bibu()
        .args(["repo", "--repo", "acme/api"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["workspace"], "acme");
    assert_eq!(v["slug"], "api");
    assert!(out.stderr.is_empty());
}

#[test]
fn repo_from_env() {
    let out = bibu()
        .env("BIBU_REPO", "git@bitbucket.org:envws/envrepo.git")
        .arg("repo")
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["workspace"], "envws");
    assert_eq!(v["slug"], "envrepo");
}

#[test]
fn flag_beats_env() {
    let out = bibu()
        .env("BIBU_REPO", "env/ignored")
        .args(["repo", "--repo", "flag/wins"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["workspace"], "flag");
}

#[test]
fn invalid_repo_exits_2_with_json_error_on_stderr_only() {
    let out = bibu()
        .args(["repo", "--repo", "https://github.com/a/b"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "nothing on stdout on failure");
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["code"], "usage");
}

#[test]
fn no_repo_anywhere_exits_2_and_mentions_flag() {
    bibu()
        .arg("repo")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--repo"));
}

#[test]
fn unknown_subcommand_is_a_usage_error() {
    bibu().arg("nonsense").assert().code(2);
}

#[test]
fn help_and_version_succeed() {
    bibu()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Bitbucket"));
    bibu()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("bibu"));
}
