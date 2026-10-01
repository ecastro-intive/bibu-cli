//! Live tests: run `bibu` against a real Bitbucket Cloud sandbox repository.
//!
//! Ignored by default. They create branches, pull requests, comments and tasks in the repository
//! named by `BIBU_TEST_REPO`, so point them at a **throwaway** repository only.
//!
//! ```sh
//! BIBU_TEST_REPO=workspace/sandbox \
//! BIBU_TEST_EMAIL=you@company.com \
//! BIBU_TEST_TOKEN=... \
//! BIBU_TEST_ALLOW_WRITES=1 \
//!   cargo test --test live -- --ignored --test-threads=1
//! ```
//!
//! Optional: `BIBU_TEST_REVIEWER` (name or uuid of another workspace member, enables the reviewer
//! flow) and `BIBU_TEST_ALLOW_MERGE=1` (also runs the merge flow, which adds a commit to the
//! default branch).
//!
//! Every JSON output is validated against the schema `bibu schema` publishes for that command, so
//! this suite also proves the documented output shapes are the real ones. Whatever a test creates
//! is removed at the end, even when an assertion fails.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use serde_json::Value;

// ------------------------------------------------------------------- harness

struct Live {
    repo: String,
    email: String,
    token: String,
    api: String,
    run_id: String,
    scratch: tempfile::TempDir,
}

fn need(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!(
            "live tests need {name}. Example:\n  BIBU_TEST_REPO=workspace/sandbox BIBU_TEST_EMAIL=you@x.io \
             BIBU_TEST_TOKEN=... BIBU_TEST_ALLOW_WRITES=1 cargo test --test live -- --ignored --test-threads=1"
        )
    })
}

impl Live {
    fn new() -> Self {
        assert_eq!(
            need("BIBU_TEST_ALLOW_WRITES"),
            "1",
            "set BIBU_TEST_ALLOW_WRITES=1 to confirm BIBU_TEST_REPO is a throwaway repository"
        );
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        Self {
            repo: need("BIBU_TEST_REPO"),
            email: need("BIBU_TEST_EMAIL"),
            token: need("BIBU_TEST_TOKEN"),
            api: std::env::var("BIBU_API_BASE")
                .unwrap_or_else(|_| bibu::api::DEFAULT_API_BASE.to_string()),
            run_id: format!("{}-{}", millis % 100_000_000, std::process::id()),
            scratch: tempfile::tempdir().unwrap(),
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("bibu").unwrap();
        cmd.env("BIBU_EMAIL", &self.email)
            .env("BIBU_TOKEN", &self.token)
            .env("BIBU_REPO", &self.repo)
            .env_remove("BIBU_API_BASE")
            .env(
                "BIBU_CREDENTIALS_FILE",
                self.scratch.path().join("creds.json"),
            )
            .current_dir(self.scratch.path());
        if self.api != bibu::api::DEFAULT_API_BASE {
            cmd.env("BIBU_API_BASE", &self.api);
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        self.cmd().args(args).output().unwrap()
    }

    /// Runs a command that must succeed; returns its JSON output after checking it against the
    /// schema published for that command.
    fn ok(&self, args: &[&str]) -> Value {
        let mut full = args.to_vec();
        full.push("--json");
        let out = self.run(&full);
        assert!(
            out.status.success(),
            "`bibu {}` failed with {:?}: {}",
            args.join(" "),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        let value: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "`bibu {}` printed non-JSON ({e}): {:?}",
                args.join(" "),
                String::from_utf8_lossy(&out.stdout)
            )
        });
        assert_matches_schema(args, &value);
        value
    }

    /// Runs a command that must fail with `code`; returns the error envelope.
    fn fails(&self, args: &[&str], code: i32) -> Value {
        let mut full = args.to_vec();
        full.push("--json");
        let out = self.run(&full);
        assert_eq!(
            out.status.code(),
            Some(code),
            "`bibu {}` should exit {code}: stdout={:?} stderr={:?}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stdout.is_empty(),
            "nothing on stdout when a command fails"
        );
        let envelope: Value = serde_json::from_slice(&out.stderr).expect("error envelope is JSON");
        let schema = bibu::schema::error_schema();
        assert!(
            jsonschema::validator_for(&schema)
                .unwrap()
                .is_valid(&envelope),
            "{envelope}"
        );
        envelope
    }

    fn name(&self, what: &str) -> String {
        format!("live-{what}-{}", self.run_id)
    }

    fn default_branch(&self) -> (String, String) {
        let branches = self.ok(&["branch", "list", "--all"]);
        let default = branches
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["default"] == true)
            .expect("the repository has a default branch");
        (
            default["name"].as_str().unwrap().to_string(),
            default["hash"].as_str().unwrap().to_string(),
        )
    }

    /// Commits `files` to `branch` (creating it from `parent`) through the REST API, because
    /// `git push` may be unavailable on a sandbox and a form body is all the endpoint needs.
    fn commit(&self, branch: &str, parent: Option<&str>, files: &[(&str, &str)], message: &str) {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        for (path, content) in files {
            form.append_pair(path, content);
        }
        form.append_pair("message", message);
        form.append_pair("branch", branch);
        if let Some(parent) = parent {
            form.append_pair("parents", parent);
        }
        let response = reqwest::blocking::Client::new()
            .post(format!("{}/repositories/{}/src", self.api, self.repo))
            .basic_auth(&self.email, Some(&self.token))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form.finish())
            .send()
            .expect("commit request");
        assert_eq!(
            response.status().as_u16(),
            201,
            "commit failed: {}",
            response.text().unwrap_or_default()
        );
    }
}

/// Finds the command a `bibu` invocation runs (following names and aliases) and checks `value`
/// against the output schema published for it.
fn assert_matches_schema(args: &[&str], value: &Value) {
    let root = bibu::schema::tree();
    let mut path: Vec<String> = Vec::new();
    for arg in args {
        let mut probe = path.clone();
        probe.push((*arg).to_string());
        if bibu::schema::find(&root, &probe).is_ok() {
            path = probe;
        } else {
            break;
        }
    }
    let command = bibu::schema::find(&root, &path).expect("known command");
    let output = command
        .output
        .as_ref()
        .unwrap_or_else(|| panic!("`bibu {}` has no output schema", path.join(" ")));
    let validator = jsonschema::validator_for(&output.schema).expect("schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "`bibu {}` output does not match its schema: {errors:?}\n{value}",
        path.join(" ")
    );
}

/// Removes what a test created, whether or not it passed.
struct Cleanup<'a> {
    live: &'a Live,
    branches: RefCell<Vec<String>>,
    pull_requests: RefCell<Vec<u64>>,
}

impl<'a> Cleanup<'a> {
    fn new(live: &'a Live) -> Self {
        Self {
            live,
            branches: RefCell::default(),
            pull_requests: RefCell::default(),
        }
    }
    fn branch(&self, name: &str) {
        self.branches.borrow_mut().push(name.to_string());
    }
    fn pull_request(&self, id: u64) {
        self.pull_requests.borrow_mut().push(id);
    }
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for id in self.pull_requests.borrow().iter() {
            let _ = self.live.run(&["pr", "decline", &id.to_string(), "--yes"]);
        }
        for branch in self.branches.borrow().iter() {
            let _ = self.live.run(&["branch", "delete", branch, "--yes"]);
        }
    }
}

fn ids(list: &Value) -> Vec<u64> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_u64().unwrap())
        .collect()
}

// ----------------------------------------------------------------------- tests

#[test]
#[ignore = "talks to a real Bitbucket repository"]
fn auth_and_repository() {
    let live = Live::new();
    let status = live.ok(&["auth", "status"]);
    assert_eq!(status["source"], "env");
    assert_eq!(status["email"], live.email.as_str());
    assert!(status["account"]["uuid"].as_str().unwrap().starts_with('{'));

    let repo = live.ok(&["repo"]);
    assert_eq!(
        format!(
            "{}/{}",
            repo["workspace"].as_str().unwrap(),
            repo["slug"].as_str().unwrap()
        ),
        live.repo
    );

    // a wrong token is an auth error (exit 3) with a hint
    let out = live
        .cmd()
        .env("BIBU_TOKEN", "definitely-not-a-token")
        .args(["auth", "status", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stderr).unwrap()["error"]["code"],
        "auth_invalid"
    );
}

#[test]
#[ignore = "talks to a real Bitbucket repository"]
fn branch_lifecycle() {
    let live = Live::new();
    let cleanup = Cleanup::new(&live);
    let (default_name, default_hash) = live.default_branch();

    let from_default = live.name("default");
    let created = live.ok(&["branch", "create", &from_default]);
    cleanup.branch(&from_default);
    assert_eq!(created["name"], from_default.as_str());
    assert_eq!(created["hash"], default_hash.as_str());

    let from_hash = live.name("hash");
    let created = live.ok(&[
        "branch",
        "create",
        &from_hash,
        "--from",
        &default_hash[..10],
    ]);
    cleanup.branch(&from_hash);
    assert_eq!(created["hash"], default_hash.as_str());

    let from_branch = format!("{}/nested-{}", "live-slash", live.run_id);
    live.ok(&["branch", "create", &from_branch, "--from", &from_default]);
    cleanup.branch(&from_branch);

    live.fails(&["branch", "create", &from_default], 6);
    live.fails(
        &[
            "branch",
            "create",
            &live.name("ghost"),
            "--from",
            "no-such-branch-here",
        ],
        5,
    );

    let found = live.ok(&["branch", "list", "--name", &live.run_id]);
    assert_eq!(found.as_array().unwrap().len(), 3, "{found}");
    assert!(found
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["default"] == false));

    // destructive: needs --yes without a terminal, and never touches the default branch
    live.fails(&["branch", "delete", &from_default], 2);
    live.fails(&["branch", "delete", &default_name, "--yes"], 6);
    let deleted = live.ok(&["branch", "delete", &from_default, "--yes"]);
    assert_eq!(deleted["action"], "deleted");
    let left = live.ok(&["branch", "list", "--name", &from_default]);
    assert!(left.as_array().unwrap().is_empty());
}

#[test]
#[ignore = "talks to a real Bitbucket repository"]
fn pull_request_review_flow() {
    let live = Live::new();
    let cleanup = Cleanup::new(&live);
    let (default_name, default_hash) = live.default_branch();

    // a branch with one new file
    let branch = live.name("pr");
    let file = format!("live/{}.txt", live.run_id);
    live.commit(
        &branch,
        Some(&default_hash),
        &[(&file, "one\ntwo\nthree\nfour\nfive\nsix\n")],
        "chore: live test file",
    );
    cleanup.branch(&branch);

    // ---- open and inspect
    let created = live.ok(&[
        "pr",
        "create",
        "-s",
        &branch,
        "-d",
        &default_name,
        "-t",
        "Live test PR",
        "-m",
        "Created by the live suite.",
        "--no-default-reviewers",
    ]);
    let id = created["pull_requests"][0]["id"].as_u64().unwrap();
    cleanup.pull_request(id);
    let id_s = id.to_string();
    let id_s = id_s.as_str();

    let view = live.ok(&["pr", "view", id_s]);
    assert_eq!(view["title"], "Live test PR");
    assert_eq!(view["description"], "Created by the live suite.");
    assert_eq!(view["state"], "OPEN");
    assert_eq!(view["source_branch"], branch.as_str());
    assert!(ids(&live.ok(&["pr", "list"])).contains(&id));
    assert!(ids(&live.ok(&["pr", "list", "--author", "me"])).contains(&id));
    assert!(ids(&live.ok(&["pr", "list", "--source", &branch])).contains(&id));

    let edited = live.ok(&[
        "pr",
        "edit",
        id_s,
        "--title",
        "Live test PR (edited)",
        "--draft",
    ]);
    assert_eq!(
        (
            edited["title"].as_str().unwrap(),
            edited["draft"].as_bool().unwrap()
        ),
        ("Live test PR (edited)", true)
    );
    assert_eq!(live.ok(&["pr", "edit", id_s, "--ready"])["draft"], false);
    live.fails(&["pr", "edit", id_s], 2);

    let files = live.ok(&["pr", "files", id_s]);
    assert!(
        files
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == file.as_str() && f["status"] == "added"),
        "{files}"
    );
    assert_eq!(
        live.ok(&["pr", "commits", id_s]).as_array().unwrap().len(),
        1
    );
    let diff = live.ok(&["pr", "diff", id_s]);
    assert!(
        diff["diff"].as_str().unwrap().contains(&file)
            && diff["diff"].as_str().unwrap().contains("+three")
    );
    let raw = live.run(&["pr", "diff", id_s]);
    assert!(
        String::from_utf8_lossy(&raw.stdout).starts_with("diff --git"),
        "raw diff even when piped"
    );

    // ---- comments: every anchor kind, threads, resolving
    let general = live.ok(&["pr", "comment", "add", id_s, "General remark"]);
    assert!(general["inline"].is_null());
    let on_line = live.ok(&[
        "pr",
        "comment",
        "add",
        id_s,
        "--file",
        &file,
        "--line",
        "2",
        "About line two",
    ]);
    assert_eq!(
        (
            on_line["inline"]["line"].as_u64(),
            on_line["inline"]["side"].as_str()
        ),
        (Some(2), Some("new"))
    );
    let on_range = live.ok(&[
        "pr",
        "comment",
        "add",
        id_s,
        "--file",
        &file,
        "--line",
        "3",
        "--end-line",
        "5",
        "A range",
    ]);
    assert_eq!(
        (
            on_range["inline"]["line"].as_u64(),
            on_range["inline"]["end_line"].as_u64()
        ),
        (Some(3), Some(5))
    );
    let on_file = live.ok(&["pr", "comment", "add", id_s, "--file", &file, "Whole file"]);
    assert_eq!(on_file["inline"]["side"], "file");
    let old_side = live.ok(&[
        "pr",
        "comment",
        "add",
        id_s,
        "--file",
        "README.md",
        "--line",
        "1",
        "--old-side",
        "Old side",
    ]);
    assert_eq!(old_side["inline"]["side"], "old");
    let stdin = live
        .cmd()
        .args(["pr", "comment", "add", id_s, "--json"])
        .write_stdin("from stdin\nsecond line\n")
        .output()
        .unwrap();
    assert!(stdin.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&stdin.stdout).unwrap()["content"],
        "from stdin\nsecond line"
    );

    let line_id = on_line["id"].as_u64().unwrap();
    let line_id_s = line_id.to_string();
    let line_id_s = line_id_s.as_str();
    let reply = live.ok(&["pr", "comment", "reply", id_s, line_id_s, "Replying"]);
    assert_eq!(reply["parent_id"], line_id);
    let edited = live.ok(&[
        "pr",
        "comment",
        "edit",
        id_s,
        &reply["id"].to_string(),
        "Replying (edited)",
    ]);
    assert_eq!(edited["content"], "Replying (edited)");

    let all = live.ok(&["pr", "comment", "list", id_s]);
    assert!(ids(&all).len() >= 7);
    assert!(ids(&live.ok(&["pr", "comment", "list", id_s, "--file", &file])).contains(&line_id));
    assert!(
        !ids(&live.ok(&["pr", "comment", "list", id_s, "--file", &file]))
            .contains(&general["id"].as_u64().unwrap())
    );

    live.ok(&["pr", "comment", "resolve", id_s, line_id_s]);
    let open = ids(&live.ok(&["pr", "comment", "list", id_s, "--unresolved"]));
    assert!(
        !open.contains(&line_id) && !open.contains(&reply["id"].as_u64().unwrap()),
        "a resolved thread drops out with its replies"
    );
    live.fails(&["pr", "comment", "resolve", id_s, line_id_s], 6);
    live.ok(&["pr", "comment", "reopen", id_s, line_id_s]);
    live.fails(&["pr", "comment", "reopen", id_s, line_id_s], 5);
    assert!(ids(&live.ok(&["pr", "comment", "list", id_s, "--unresolved"])).contains(&line_id));

    let doomed = live.ok(&["pr", "comment", "add", id_s, "to be deleted"])["id"]
        .as_u64()
        .unwrap()
        .to_string();
    live.fails(&["pr", "comment", "delete", id_s, &doomed], 2);
    live.ok(&["pr", "comment", "delete", id_s, &doomed, "--yes"]);
    assert!(!ids(&live.ok(&["pr", "comment", "list", id_s]))
        .iter()
        .any(|c| c.to_string() == doomed));
    assert!(
        ids(&live.ok(&["pr", "comment", "list", id_s, "--include-deleted"]))
            .iter()
            .any(|c| c.to_string() == doomed)
    );

    // ---- tasks
    let task = live.ok(&["pr", "task", "add", id_s, "Standalone task"]);
    let attached = live.ok(&[
        "pr",
        "task",
        "add",
        id_s,
        "Task on a comment",
        "--comment",
        line_id_s,
    ]);
    assert_eq!(attached["comment_id"], line_id);
    let task_id = task["id"].as_u64().unwrap().to_string();
    assert_eq!(
        live.ok(&["pr", "task", "list", id_s])
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        live.ok(&["pr", "task", "resolve", id_s, &task_id])["state"],
        "RESOLVED"
    );
    assert_eq!(
        live.ok(&["pr", "task", "list", id_s, "--unresolved"])
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        live.ok(&["pr", "task", "reopen", id_s, &task_id])["state"],
        "UNRESOLVED"
    );

    // ---- reviewers (the author can never review their own pull request)
    assert!(live.ok(&["pr", "reviewers", "list", id_s])["reviewers"]
        .as_array()
        .unwrap()
        .is_empty());
    live.fails(&["pr", "reviewers", "add", id_s, "me"], 6);
    live.fails(&["pr", "reviewers", "add", id_s, ""], 2);
    // An unset CI secret arrives as an empty string: that means "not configured".
    match std::env::var("BIBU_TEST_REVIEWER")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(reviewer) => {
            let added = live.ok(&["pr", "reviewers", "add", id_s, &reviewer]);
            assert_eq!(added["reviewers"].as_array().unwrap().len(), 1, "{added}");
            let again = live.ok(&["pr", "reviewers", "add", id_s, &reviewer]);
            assert_eq!(
                again["reviewers"].as_array().unwrap().len(),
                1,
                "adding twice changes nothing"
            );
            assert_eq!(
                live.ok(&["pr", "view", id_s])["title"],
                "Live test PR (edited)",
                "updating reviewers keeps the other fields"
            );
            let removed = live.ok(&["pr", "reviewers", "remove", id_s, &reviewer]);
            assert!(removed["reviewers"].as_array().unwrap().is_empty());
        }
        None => eprintln!("note: BIBU_TEST_REVIEWER not set, reviewer add/remove not exercised"),
    }

    // ---- review decisions
    live.ok(&["pr", "approve", id_s]);
    live.ok(&["pr", "unapprove", id_s]);
    live.ok(&["pr", "request-changes", id_s]);
    let states = live.ok(&["pr", "view", id_s]);
    assert!(
        states["participants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["state"] == "changes_requested"),
        "{states}"
    );
    live.ok(&["pr", "unrequest-changes", id_s]);

    // ---- decline: destructive, needs --yes
    live.fails(&["pr", "decline", id_s], 2);
    let declined = live.ok(&["pr", "decline", id_s, "--yes"]);
    assert_eq!(declined["state"], "DECLINED");
    assert!(ids(&live.ok(&["pr", "list", "--state", "declined"])).contains(&id));
    live.fails(&["pr", "view", "999999999"], 5);
}

#[test]
#[ignore = "talks to a real Bitbucket repository and adds a commit to the default branch"]
fn merge_flow() {
    if std::env::var("BIBU_TEST_ALLOW_MERGE").as_deref() != Ok("1") {
        eprintln!("note: set BIBU_TEST_ALLOW_MERGE=1 to run the merge flow (it adds a commit to the default branch)");
        return;
    }
    let live = Live::new();
    let cleanup = Cleanup::new(&live);
    let (default_name, default_hash) = live.default_branch();
    let branch = live.name("merge");
    live.commit(
        &branch,
        Some(&default_hash),
        &[(&format!("live/{}.txt", live.run_id), "merge me\n")],
        "chore: live merge file",
    );
    cleanup.branch(&branch);

    let id = live.ok(&[
        "pr",
        "create",
        "-s",
        &branch,
        "-d",
        &default_name,
        "--no-default-reviewers",
    ])["pull_requests"][0]["id"]
        .as_u64()
        .unwrap();
    cleanup.pull_request(id);
    let id_s = id.to_string();
    live.fails(&["pr", "merge", &id_s], 2);
    let merged = live.ok(&[
        "pr",
        "merge",
        &id_s,
        "--yes",
        "--strategy",
        "squash",
        "--close-source-branch",
        "-m",
        "chore: live merge",
    ]);
    assert_eq!(merged["state"], "MERGED");
    assert!(merged["merge_commit"]
        .as_str()
        .is_some_and(|h| !h.is_empty()));
    let left = live.ok(&["branch", "list", "--name", &branch]);
    assert!(
        left.as_array().unwrap().is_empty(),
        "the source branch is closed on merge"
    );
}

#[test]
#[ignore = "talks to a real Bitbucket repository"]
fn pipelines_read_only() {
    let live = Live::new();
    let runs = live.ok(&["pipeline", "list", "--limit", "10"]);
    let runs = runs.as_array().unwrap();
    if runs.is_empty() {
        eprintln!("note: the repository has no pipeline runs, nothing to read");
        return;
    }
    let newest = &runs[0];
    let number = newest["build_number"].as_u64().unwrap().to_string();
    let by_number = live.ok(&["pipeline", "view", &number]);
    let by_uuid = live.ok(&["pipeline", "view", newest["uuid"].as_str().unwrap()]);
    assert_eq!(by_number["uuid"], by_uuid["uuid"]);
    assert_eq!(by_number["uuid"], newest["uuid"]);

    let branch = newest["branch"].as_str().unwrap();
    let on_branch = live.ok(&["pipeline", "list", "--branch", branch]);
    assert!(on_branch
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["branch"] == branch));

    let status = newest["status"].as_str().unwrap();
    if let Some(known) = [
        "pending",
        "parsing",
        "running",
        "paused",
        "halted",
        "successful",
        "failed",
        "stopped",
        "error",
    ]
    .iter()
    .find(|s| **s == status)
    {
        let same = live.ok(&["pipeline", "list", "--status", known, "--limit", "10"]);
        assert!(same
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["status"] == *known));
    }

    let steps = by_number["steps"].as_array().unwrap();
    if !steps.is_empty() {
        let logs = live.ok(&["pipeline", "logs", &number]);
        assert_eq!(logs["steps"].as_array().unwrap().len(), steps.len());
        let first = steps[0]["name"].as_str().unwrap();
        let one = live.ok(&["pipeline", "logs", &number, "--step", first]);
        assert_eq!(one["steps"].as_array().unwrap().len(), 1);
        let raw = live.run(&["pipeline", "logs", &number, "--step", "1"]);
        assert!(raw.status.success() && !raw.stdout.is_empty());
    }
    live.fails(&["pipeline", "view", "999999999"], 5);
}

#[test]
#[ignore = "talks to a real Bitbucket repository"]
fn members() {
    let live = Live::new();
    let me = live.ok(&["auth", "status"])["account"].clone();
    let everyone = live.ok(&["member", "list", "--all"]);
    assert!(
        everyone
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["uuid"] == me["uuid"]),
        "the current user is a workspace member"
    );

    let name = me["display_name"].as_str().unwrap();
    let found = live.ok(&["member", "find", name]);
    assert!(found
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["uuid"] == me["uuid"]));
    assert!(live
        .ok(&["member", "find", "zzz-no-such-person-zzz"])
        .as_array()
        .unwrap()
        .is_empty());
}
