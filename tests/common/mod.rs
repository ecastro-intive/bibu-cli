//! Helpers shared by the end-to-end tests: a mock Bitbucket API plus an isolated `bibu` command.

#![allow(dead_code)]

use assert_cmd::Command;
use base64::Engine;
use mockito::{Matcher, Mock, ServerGuard};
use serde_json::{json, Value};
use tempfile::TempDir;

pub const REPO: &str = "/repositories/acme/api";

pub struct Env {
    pub server: ServerGuard,
    pub dir: TempDir,
}

impl Env {
    pub fn new() -> Self {
        Self {
            server: mockito::Server::new(),
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn bibu(&self) -> Command {
        let mut cmd = Command::cargo_bin("bibu").unwrap();
        cmd.env("BIBU_EMAIL", "me@x.io")
            .env("BIBU_TOKEN", "tok")
            .env("BIBU_REPO", "acme/api")
            .env("BIBU_API_BASE", self.server.url())
            .env("BIBU_CREDENTIALS_FILE", self.dir.path().join("creds.json"))
            .current_dir(self.dir.path());
        cmd
    }

    /// A mock that only matches requests carrying the expected credentials.
    pub fn mock(&mut self, method: &str, path: &str) -> Mock {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("me@x.io:tok")
        );
        self.server
            .mock(method, path)
            .match_header("authorization", header.as_str())
            .expect(1)
    }

    pub fn json(&mut self, method: &str, path: &str, body: Value) -> Mock {
        self.mock(method, path)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create()
    }

    pub fn me(&mut self) -> Mock {
        self.json(
            "GET",
            "/user",
            json!({"display_name": "Me Myself", "uuid": "{me}", "nickname": "me"}),
        )
    }
}

pub fn account(name: &str, uuid: &str) -> Value {
    json!({"display_name": name, "uuid": uuid, "nickname": name.to_lowercase().replace(' ', "")})
}

pub fn page(values: Vec<Value>) -> Value {
    json!({"values": values})
}

pub fn stdout_json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}): {:?}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

pub fn stderr_json(out: &std::process::Output) -> Value {
    serde_json::from_slice(&out.stderr).unwrap_or_else(|e| {
        panic!(
            "stderr is not JSON ({e}): {:?}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

pub fn query(pairs: &[(&str, &str)]) -> Matcher {
    Matcher::AllOf(
        pairs
            .iter()
            .map(|(k, v)| Matcher::UrlEncoded((*k).into(), (*v).into()))
            .collect(),
    )
}
