//! End-to-end tests for `bibu auth`, run against a mock Bitbucket API.
//!
//! Every test points `BIBU_CREDENTIALS_FILE` at a temp dir so the real keychain is never used.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use base64::Engine;
use serde_json::{json, Value};
use tempfile::TempDir;

struct Env {
    server: mockito::ServerGuard,
    dir: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            server: mockito::Server::new(),
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn creds_file(&self) -> PathBuf {
        self.dir.path().join("creds.json")
    }

    fn bibu(&self) -> Command {
        let mut cmd = Command::cargo_bin("bibu").unwrap();
        cmd.env_remove("BIBU_EMAIL")
            .env_remove("BIBU_TOKEN")
            .env_remove("BIBU_REPO")
            .env("BIBU_API_BASE", self.server.url())
            .env("BIBU_CREDENTIALS_FILE", self.creds_file())
            .current_dir(self.dir.path());
        cmd
    }

    fn mock_user(&mut self, email: &str, token: &str) -> mockito::Mock {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"))
        );
        self.server
            .mock("GET", "/user")
            .match_header("authorization", header.as_str())
            .with_body(
                json!({"display_name": "Jane Doe", "uuid": "{u-1}", "nickname": "jane"})
                    .to_string(),
            )
            .create()
    }

    fn mock_user_status(&mut self, status: usize) -> mockito::Mock {
        self.server
            .mock("GET", "/user")
            .with_status(status)
            .with_body(r#"{"type":"error","error":{"message":"nope"}}"#)
            .create()
    }
}

fn stdout_json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {:?}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

fn stderr_json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stderr).unwrap_or_else(|e| {
        panic!(
            "stderr is not JSON ({e}): {:?}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn read_creds(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn login_with_token_on_stdin_stores_and_reports() {
    let mut env = Env::new();
    let mock = env.mock_user("me@x.io", "s3cret-token");

    let out = env
        .bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("s3cret-token\n")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v["email"], "me@x.io");
    assert_eq!(v["account"]["display_name"], "Jane Doe");
    assert_eq!(v["stored_in"], "file");
    assert_eq!(
        read_creds(&env.creds_file()),
        json!({"email": "me@x.io", "token": "s3cret-token"})
    );
}

#[test]
fn the_token_is_never_printed() {
    let mut env = Env::new();
    env.mock_user("me@x.io", "s3cret-token");

    let out = env
        .bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("s3cret-token")
        .output()
        .unwrap();
    let status = env.bibu().args(["auth", "status"]).output().unwrap();

    for output in [out, status] {
        assert!(!String::from_utf8_lossy(&output.stdout).contains("s3cret-token"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("s3cret-token"));
    }
}

#[cfg(unix)]
#[test]
fn stored_credentials_file_is_private() {
    use std::os::unix::fs::PermissionsExt;

    let mut env = Env::new();
    env.mock_user("me@x.io", "t");
    env.bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("t")
        .assert()
        .success();

    let mode = std::fs::metadata(env.creds_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn rejected_token_exits_3_and_stores_nothing() {
    let mut env = Env::new();
    let mock = env.mock_user_status(401);

    let out = env
        .bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("bad")
        .output()
        .unwrap();

    mock.assert();
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    assert_eq!(stderr_json(&out)["error"]["code"], "auth_invalid");
    assert!(!env.creds_file().exists());
}

#[test]
fn username_instead_of_email_exits_2_without_calling_the_api() {
    let mut env = Env::new();
    let mock = env.server.mock("GET", "/user").expect(0).create();

    let out = env
        .bibu()
        .args(["auth", "login", "--email", "jdoe", "--with-token"])
        .write_stdin("t")
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Atlassian account email"));
    mock.assert();
}

#[test]
fn login_without_a_terminal_or_with_token_exits_2() {
    let env = Env::new();
    let out = env
        .bibu()
        .args(["auth", "login", "--email", "me@x.io"])
        .write_stdin("")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--with-token"));
}

#[test]
fn login_without_an_email_exits_2() {
    let env = Env::new();
    let out = env
        .bibu()
        .args(["auth", "login", "--with-token"])
        .write_stdin("t")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--email"));
}

#[test]
fn login_with_empty_stdin_exits_2() {
    let env = Env::new();
    let out = env
        .bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn status_uses_the_stored_login() {
    let mut env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"stored"}"#).unwrap();
    let mock = env.mock_user("me@x.io", "stored");

    let out = env.bibu().args(["auth", "status"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v["authenticated"], true);
    assert_eq!(v["source"], "file");
    assert_eq!(v["email"], "me@x.io");
    assert_eq!(v["account"]["nickname"], "jane");
}

#[test]
fn env_credentials_win_over_the_stored_login() {
    let mut env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"stored"}"#).unwrap();
    let mock = env.mock_user("ci@x.io", "ci-token");

    let out = env
        .bibu()
        .env("BIBU_EMAIL", "ci@x.io")
        .env("BIBU_TOKEN", "ci-token")
        .args(["auth", "status"])
        .output()
        .unwrap();

    mock.assert();
    assert_eq!(stdout_json(&out)["source"], "env");
}

#[test]
fn half_set_env_credentials_exit_2() {
    let env = Env::new();
    let out = env
        .bibu()
        .env("BIBU_TOKEN", "t")
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("together"));
}

#[test]
fn status_when_logged_out_exits_3_with_a_hint() {
    let env = Env::new();
    let out = env.bibu().args(["auth", "status"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(out.stdout.is_empty());
    let v = stderr_json(&out);
    assert_eq!(v["error"]["code"], "auth_invalid");
    assert!(v["error"]["hint"]
        .as_str()
        .unwrap()
        .contains("bibu auth login"));
}

#[test]
fn status_with_a_revoked_token_exits_3() {
    let mut env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"dead"}"#).unwrap();
    env.mock_user_status(401);
    let out = env.bibu().args(["auth", "status"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
}

#[test]
fn status_with_a_token_lacking_scopes_exits_4_and_names_them() {
    let mut env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"t"}"#).unwrap();
    env.server
        .mock("GET", "/user")
        .with_status(403)
        .with_body(r#"{"type":"error","error":{"message":"Your credentials lack one or more required privilege scopes.","detail":{"required":["read:user:bitbucket"]}}}"#)
        .create();

    let out = env.bibu().args(["auth", "status"]).output().unwrap();

    assert_eq!(out.status.code(), Some(4));
    let v = stderr_json(&out);
    assert_eq!(v["error"]["code"], "forbidden");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read:user:bitbucket"));
}

#[test]
fn unreachable_api_exits_7() {
    let env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"t"}"#).unwrap();
    let out = env
        .bibu()
        .env("BIBU_API_BASE", "http://127.0.0.1:1")
        .args(["auth", "status"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(stderr_json(&out)["error"]["code"], "network");
}

#[test]
fn corrupt_credentials_file_exits_1_with_a_fix() {
    let env = Env::new();
    std::fs::write(env.creds_file(), "garbage").unwrap();
    let out = env.bibu().args(["auth", "status"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("bibu auth login"));
}

#[test]
fn logout_removes_the_file_and_is_idempotent() {
    let env = Env::new();
    std::fs::write(env.creds_file(), r#"{"email":"me@x.io","token":"t"}"#).unwrap();

    let first = env.bibu().args(["auth", "logout"]).output().unwrap();
    let second = env.bibu().args(["auth", "logout"]).output().unwrap();

    assert!(first.status.success() && second.status.success());
    assert_eq!(stdout_json(&first)["removed"], true);
    assert_eq!(stdout_json(&second)["removed"], false);
    assert!(!env.creds_file().exists());
}

#[test]
fn logout_flags_env_credentials_that_remain() {
    let env = Env::new();
    let out = env
        .bibu()
        .env("BIBU_EMAIL", "a@b.io")
        .env("BIBU_TOKEN", "t")
        .args(["auth", "logout"])
        .output()
        .unwrap();
    assert_eq!(stdout_json(&out)["env_credentials_still_set"], true);
}

#[test]
fn login_then_status_round_trip() {
    let mut env = Env::new();
    env.mock_user("me@x.io", "round-trip");

    env.bibu()
        .args(["auth", "login", "--email", "me@x.io", "--with-token"])
        .write_stdin("round-trip")
        .assert()
        .success();
    let out = env.bibu().args(["auth", "status"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout_json(&out)["account"]["display_name"], "Jane Doe");
}
