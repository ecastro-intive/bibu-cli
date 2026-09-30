//! End-to-end tests for `bibu pr ...` against a mock Bitbucket API.
//!
//! Each mock asserts the exact method, path, query, JSON body and `Authorization` header the
//! CLI sends, and `assert()` proves it was called the expected number of times.

use assert_cmd::Command;
use base64::Engine;
use mockito::{Matcher, Mock, ServerGuard};
use serde_json::{json, Value};
use tempfile::TempDir;

const REPO: &str = "/repositories/acme/api";

struct Env {
    server: ServerGuard,
    dir: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            server: mockito::Server::new(),
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn bibu(&self) -> Command {
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
    fn mock(&mut self, method: &str, path: &str) -> Mock {
        let header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("me@x.io:tok")
        );
        self.server
            .mock(method, path)
            .match_header("authorization", header.as_str())
            .expect(1)
    }

    fn json(&mut self, method: &str, path: &str, body: Value) -> Mock {
        self.mock(method, path)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create()
    }

    fn me(&mut self) -> Mock {
        self.json(
            "GET",
            "/user",
            json!({"display_name": "Me Myself", "uuid": "{me}", "nickname": "me"}),
        )
    }
}

fn account(name: &str, uuid: &str) -> Value {
    json!({"display_name": name, "uuid": uuid, "nickname": name.to_lowercase().replace(' ', "")})
}

fn pr_json(id: u64, title: &str, src: &str, dst: &str, author: Value) -> Value {
    json!({
        "id": id, "title": title, "state": "OPEN", "draft": false,
        "author": author,
        "source": {"branch": {"name": src}},
        "destination": {"branch": {"name": dst}},
        "comment_count": 2, "task_count": 1,
        "created_on": "2026-09-01T10:00:00+00:00", "updated_on": "2026-09-02T11:00:00+00:00",
        "links": {"html": {"href": format!("https://bitbucket.org/acme/api/pull-requests/{id}")}},
    })
}

fn page(values: Vec<Value>) -> Value {
    json!({"values": values})
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

fn query(pairs: &[(&str, &str)]) -> Matcher {
    Matcher::AllOf(
        pairs
            .iter()
            .map(|(k, v)| Matcher::UrlEncoded((*k).into(), (*v).into()))
            .collect(),
    )
}

// ------------------------------------------------------------------------ list

#[test]
fn list_defaults_to_open_with_a_page_of_25() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(query(&[("state", "OPEN"), ("pagelen", "25")]))
        .with_body(
            page(vec![pr_json(
                7,
                "Fix bug",
                "fix/x",
                "main",
                account("Jane Doe", "{j}"),
            )])
            .to_string(),
        )
        .create();

    let out = env.bibu().args(["pr", "list"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v[0]["id"], 7);
    assert_eq!(v[0]["title"], "Fix bug");
    assert_eq!(v[0]["source_branch"], "fix/x");
    assert_eq!(v[0]["destination_branch"], "main");
    assert_eq!(v[0]["author"]["display_name"], "Jane Doe");
    assert_eq!(
        v[0]["url"],
        "https://bitbucket.org/acme/api/pull-requests/7"
    );
    assert_eq!(v[0]["comment_count"], 2);
    assert!(v[0].get("participants").is_none());
}

#[test]
fn list_state_maps_to_the_api_spelling() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(query(&[("state", "MERGED")]))
        .with_body(page(vec![]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "list", "--state", "merged"])
        .output()
        .unwrap();

    mock.assert();
    assert_eq!(stdout_json(&out), json!([]));
}

#[test]
fn list_limit_and_all_control_paging() {
    let mut env = Env::new();
    let limited = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(query(&[("pagelen", "3")]))
        .with_body(
            page(
                (1..=5)
                    .map(|i| pr_json(i, "t", "a", "b", account("J", "{j}")))
                    .collect(),
            )
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pr", "list", "--limit", "3"])
        .output()
        .unwrap();

    limited.assert();
    assert_eq!(stdout_json(&out).as_array().unwrap().len(), 3);
}

#[test]
fn list_all_follows_next_links() {
    let mut env = Env::new();
    let next = format!("{}{REPO}/pullrequests?page=2", env.server.url());
    let first = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(query(&[("pagelen", "100")]))
        .with_body(
            json!({"values": [pr_json(1, "a", "x", "y", account("J", "{j}"))], "next": next})
                .to_string(),
        )
        .create();
    let second = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(query(&[("page", "2")]))
        .with_body(page(vec![pr_json(2, "b", "x", "y", account("J", "{j}"))]).to_string())
        .create();

    let out = env.bibu().args(["pr", "list", "--all"]).output().unwrap();

    first.assert();
    second.assert();
    let ids: Vec<u64> = stdout_json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![1, 2]);
}

#[test]
fn list_filters_by_branches_and_author_across_the_whole_result() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(Matcher::Any)
        .with_body(
            page(vec![
                pr_json(1, "wrong dest", "a", "develop", account("Jane Doe", "{j}")),
                pr_json(2, "wrong author", "a", "main", account("Bob Ray", "{b}")),
                pr_json(3, "match", "a", "main", account("Jane Doe", "{j}")),
                pr_json(4, "wrong source", "z", "main", account("Jane Doe", "{j}")),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "list",
            "--destination",
            "main",
            "--source",
            "a",
            "--author",
            "janedoe",
        ])
        .output()
        .unwrap();

    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["id"], 3);
}

#[test]
fn list_author_me_resolves_the_current_user() {
    let mut env = Env::new();
    let me = env.me();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(Matcher::Any)
        .with_body(
            page(vec![
                pr_json(1, "mine", "a", "main", account("Me Myself", "{me}")),
                pr_json(2, "theirs", "a", "main", account("Bob Ray", "{b}")),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pr", "list", "--author", "me"])
        .output()
        .unwrap();

    me.assert();
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["id"], 1);
}

#[test]
fn list_reviews_fetches_each_pr_for_participants() {
    let mut env = Env::new();
    let list = env
        .mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(Matcher::Any)
        .with_body(page(vec![pr_json(5, "t", "a", "b", account("J", "{j}"))]).to_string())
        .create();
    let mut detail = pr_json(5, "t", "a", "b", account("J", "{j}"));
    detail["participants"] = json!([
        {"user": account("Bob Ray", "{b}"), "role": "REVIEWER", "approved": true, "state": "approved"}
    ]);
    let one = env.json("GET", &format!("{REPO}/pullrequests/5"), detail);

    let out = env
        .bibu()
        .args(["pr", "list", "--reviews"])
        .output()
        .unwrap();

    list.assert();
    one.assert();
    let v = stdout_json(&out);
    assert_eq!(v[0]["participants"][0]["user"]["display_name"], "Bob Ray");
    assert_eq!(v[0]["participants"][0]["approved"], true);
}

#[test]
fn list_rejects_bad_paging_before_any_request() {
    let env = Env::new();
    for args in [
        vec!["pr", "list", "--limit", "0"],
        vec!["pr", "list", "--limit", "abc"],
        vec!["pr", "list", "--limit", "5", "--all"],
        vec!["pr", "list", "--state", "bogus"],
    ] {
        assert_eq!(
            env.bibu().args(&args).output().unwrap().status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

// ------------------------------------------------------------------------ view

#[test]
fn view_returns_description_and_participants() {
    let mut env = Env::new();
    let mut body = pr_json(
        12,
        "Add thing",
        "feat/x",
        "main",
        account("Jane Doe", "{j}"),
    );
    body["description"] = json!("Does a thing.");
    body["participants"] = json!([
        {"user": account("Bob Ray", "{b}"), "role": "REVIEWER", "approved": false, "state": "changes_requested"}
    ]);
    let mock = env.json("GET", &format!("{REPO}/pullrequests/12"), body);

    let out = env.bibu().args(["pr", "view", "12"]).output().unwrap();

    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v["id"], 12);
    assert_eq!(v["description"], "Does a thing.");
    assert_eq!(v["participants"][0]["state"], "changes_requested");
    assert_eq!(v["close_source_branch"], false);
}

#[test]
fn view_unknown_pr_exits_5() {
    let mut env = Env::new();
    env.mock("GET", &format!("{REPO}/pullrequests/99"))
        .with_status(404)
        .with_body(r#"{"type":"error","error":{"message":"No pull request found"}}"#)
        .create();

    let out = env.bibu().args(["pr", "view", "99"]).output().unwrap();

    assert_eq!(out.status.code(), Some(5));
    assert!(out.stdout.is_empty());
    let v = stderr_json(&out);
    assert_eq!(v["error"]["code"], "not_found");
    assert_eq!(v["error"]["message"], "No pull request found");
}

#[test]
fn a_non_numeric_id_is_a_usage_error() {
    let env = Env::new();
    assert_eq!(
        env.bibu()
            .args(["pr", "view", "abc"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        env.bibu()
            .args(["pr", "view"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}

// ---------------------------------------------------------------------- create

#[test]
fn create_sends_the_exact_body_with_default_reviewers_minus_me() {
    let mut env = Env::new();
    let me = env.me();
    let reviewers = env
        .mock("GET", &format!("{REPO}/effective-default-reviewers"))
        .match_query(query(&[("pagelen", "100")]))
        .with_body(
            page(vec![
                json!({"reviewer_type": "repository", "user": account("Me Myself", "{me}")}),
                json!({"reviewer_type": "repository", "user": account("Bob Ray", "{b}")}),
            ])
            .to_string(),
        )
        .create();
    let create = env
        .mock("POST", &format!("{REPO}/pullrequests"))
        .match_body(Matcher::Json(json!({
            "title": "Merge feat/x into main",
            "source": {"branch": {"name": "feat/x"}},
            "destination": {"branch": {"name": "main"}},
            "reviewers": [{"uuid": "{b}"}],
        })))
        .with_status(201)
        .with_body(
            pr_json(
                21,
                "Merge feat/x into main",
                "feat/x",
                "main",
                account("Me Myself", "{me}"),
            )
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "create",
            "--source",
            "feat/x",
            "--destination",
            "main",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    me.assert();
    reviewers.assert();
    create.assert();
    let v = stdout_json(&out);
    assert_eq!(v["pull_requests"][0]["id"], 21);
    assert_eq!(
        v["pull_requests"][0]["url"],
        "https://bitbucket.org/acme/api/pull-requests/21"
    );
}

#[test]
fn create_with_all_options_and_no_default_reviewers_makes_no_extra_calls() {
    let mut env = Env::new();
    let user = env.mock("GET", "/user").expect(0).create();
    let reviewers = env
        .mock("GET", &format!("{REPO}/effective-default-reviewers"))
        .expect(0)
        .create();
    let create = env
        .mock("POST", &format!("{REPO}/pullrequests"))
        .match_body(Matcher::Json(json!({
            "title": "My title",
            "description": "Body text",
            "source": {"branch": {"name": "feat/x"}},
            "destination": {"branch": {"name": "main"}},
            "reviewers": [],
            "draft": true,
            "close_source_branch": true,
        })))
        .with_status(201)
        .with_body(pr_json(22, "My title", "feat/x", "main", account("J", "{j}")).to_string())
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "create",
            "-s",
            "feat/x",
            "-d",
            "main",
            "-t",
            "My title",
            "-m",
            "Body text",
            "--draft",
            "--close-source-branch",
            "--no-default-reviewers",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    user.assert();
    reviewers.assert();
    create.assert();
}

#[test]
fn create_with_several_destinations_makes_one_pr_each() {
    let mut env = Env::new();
    let mut created = vec![];
    for (id, dst) in [(31_u64, "main"), (32, "develop")] {
        created.push(
            env.mock("POST", &format!("{REPO}/pullrequests"))
                .match_body(Matcher::PartialJson(json!({"destination": {"branch": {"name": dst}}, "title": format!("Merge f into {dst}")})))
                .with_status(201)
                .with_body(pr_json(id, &format!("Merge f into {dst}"), "f", dst, account("J", "{j}")).to_string())
                .create(),
        );
    }

    let out = env
        .bibu()
        .args([
            "pr",
            "create",
            "-s",
            "f",
            "-d",
            "main,develop",
            "--no-default-reviewers",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    created.iter().for_each(Mock::assert);
    let ids: Vec<u64> = stdout_json(&out)["pull_requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![31, 32]);
}

#[test]
fn create_reports_what_was_already_created_when_a_later_destination_fails() {
    let mut env = Env::new();
    env.mock("POST", &format!("{REPO}/pullrequests"))
        .match_body(Matcher::PartialJson(
            json!({"destination": {"branch": {"name": "main"}}}),
        ))
        .with_status(201)
        .with_body(pr_json(41, "t", "f", "main", account("J", "{j}")).to_string())
        .create();
    env.mock("POST", &format!("{REPO}/pullrequests"))
        .match_body(Matcher::PartialJson(
            json!({"destination": {"branch": {"name": "develop"}}}),
        ))
        .with_status(400)
        .with_body(r#"{"type":"error","error":{"message":"There are no changes to be pulled"}}"#)
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "create",
            "-s",
            "f",
            "-d",
            "main,develop",
            "--no-default-reviewers",
        ])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(6));
    let message = stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(message.contains("no changes"), "{message}");
    assert!(
        message.contains("#41") && message.contains("develop"),
        "{message}"
    );
}

#[test]
fn create_first_failure_is_a_plain_error() {
    let mut env = Env::new();
    env.mock("POST", &format!("{REPO}/pullrequests"))
        .with_status(400)
        .with_body(r#"{"type":"error","error":{"message":"Branch not found"}}"#)
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "create",
            "-s",
            "f",
            "-d",
            "main",
            "--no-default-reviewers",
        ])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(6));
    assert_eq!(stderr_json(&out)["error"]["message"], "Branch not found");
}

#[test]
fn create_outside_git_without_source_exits_2() {
    let env = Env::new();
    let out = env
        .bibu()
        .args(["pr", "create", "-d", "main"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--source"));
}

#[test]
fn create_requires_a_destination() {
    let env = Env::new();
    assert_eq!(
        env.bibu()
            .args(["pr", "create", "-s", "f"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}

// ------------------------------------------------------------------------ edit

#[test]
fn edit_sends_only_the_changed_fields() {
    let mut env = Env::new();
    let mock = env
        .mock("PUT", &format!("{REPO}/pullrequests/8"))
        .match_body(Matcher::Json(json!({"title": "New title", "destination": {"branch": {"name": "develop"}}, "draft": false})))
        .with_body(pr_json(8, "New title", "f", "develop", account("J", "{j}")).to_string())
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "edit",
            "8",
            "--title",
            "New title",
            "--destination",
            "develop",
            "--ready",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    assert_eq!(stdout_json(&out)["title"], "New title");
}

#[test]
fn edit_with_no_changes_exits_2_without_a_request() {
    let mut env = Env::new();
    let never = env
        .mock("PUT", &format!("{REPO}/pullrequests/8"))
        .expect(0)
        .create();

    let out = env.bibu().args(["pr", "edit", "8"]).output().unwrap();

    assert_eq!(out.status.code(), Some(2));
    never.assert();
}

#[test]
fn edit_rejects_contradictory_flags() {
    let env = Env::new();
    for args in [
        vec!["pr", "edit", "8", "--draft", "--ready"],
        vec![
            "pr",
            "edit",
            "8",
            "--close-source-branch",
            "--keep-source-branch",
        ],
    ] {
        assert_eq!(
            env.bibu().args(&args).output().unwrap().status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

// ------------------------------------------------------------- diff/commits/files

#[test]
fn diff_prints_the_raw_text_even_when_piped() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests/5/diff"))
        .with_body("diff --git a/x b/x\n+hello\n")
        .create();

    let out = env.bibu().args(["pr", "diff", "5"]).output().unwrap();

    mock.assert();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "diff --git a/x b/x\n+hello\n"
    );
}

#[test]
fn diff_with_json_wraps_the_text() {
    let mut env = Env::new();
    env.mock("GET", &format!("{REPO}/pullrequests/5/diff"))
        .with_body("diff --git a/x b/x\n")
        .create();

    let out = env
        .bibu()
        .args(["pr", "diff", "5", "--json"])
        .output()
        .unwrap();

    let v = stdout_json(&out);
    assert_eq!(v["pull_request"], 5);
    assert_eq!(v["diff"], "diff --git a/x b/x\n");
}

#[test]
fn commits_lists_hash_author_and_subject() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests/5/commits"))
        .match_query(query(&[("pagelen", "25")]))
        .with_body(
            page(vec![json!({
                "hash": "abcdef1234567890", "date": "2026-09-03T00:00:00+00:00",
                "message": "Add feature\n\nlong body",
                "author": {"raw": "Jane Doe <j@x.io>", "user": account("Jane Doe", "{j}")},
                "links": {"html": {"href": "https://bitbucket.org/acme/api/commits/abcdef1"}},
            })])
            .to_string(),
        )
        .create();

    let out = env.bibu().args(["pr", "commits", "5"]).output().unwrap();

    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v[0]["hash"], "abcdef1234567890");
    assert_eq!(v[0]["author"], "Jane Doe");
    assert_eq!(v[0]["subject"], "Add feature");
    assert_eq!(
        v[0]["url"],
        "https://bitbucket.org/acme/api/commits/abcdef1"
    );
}

#[test]
fn files_lists_paths_status_and_line_counts() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{REPO}/pullrequests/5/diffstat"))
        .match_query(query(&[("pagelen", "100")]))
        .with_body(
            page(vec![
                json!({"status": "modified", "lines_added": 3, "lines_removed": 1, "old": {"path": "a.rs"}, "new": {"path": "a.rs"}}),
                json!({"status": "removed", "lines_added": 0, "lines_removed": 9, "old": {"path": "gone.rs"}, "new": null}),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pr", "files", "5", "--all"])
        .output()
        .unwrap();

    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(
        v[0],
        json!({"path": "a.rs", "status": "modified", "lines_added": 3, "lines_removed": 1})
    );
    assert_eq!(v[1]["path"], "gone.rs");
    assert_eq!(v[1]["status"], "removed");
}

// ------------------------------------------------------------------ review actions

#[test]
fn review_actions_hit_the_right_method_and_path() {
    let cases = [
        (vec!["approve", "5"], "POST", "approve", "approved", 200),
        (
            vec!["unapprove", "5"],
            "DELETE",
            "approve",
            "approval_removed",
            204,
        ),
        (
            vec!["no-approve", "5"],
            "DELETE",
            "approve",
            "approval_removed",
            204,
        ),
        (
            vec!["request-changes", "5"],
            "POST",
            "request-changes",
            "changes_requested",
            200,
        ),
        (
            vec!["unrequest-changes", "5"],
            "DELETE",
            "request-changes",
            "change_request_removed",
            204,
        ),
        (
            vec!["no-request-changes", "5"],
            "DELETE",
            "request-changes",
            "change_request_removed",
            204,
        ),
    ];
    for (args, method, suffix, action, status) in cases {
        let mut env = Env::new();
        let mock = env
            .mock(method, &format!("{REPO}/pullrequests/5/{suffix}"))
            .with_status(status)
            .with_body(if status == 204 {
                String::new()
            } else {
                json!({"approved": true}).to_string()
            })
            .create();

        let mut full = vec!["pr"];
        full.extend(args.iter().copied());
        let out = env.bibu().args(&full).output().unwrap();

        assert!(
            out.status.success(),
            "{full:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        mock.assert();
        let v = stdout_json(&out);
        assert_eq!(v, json!({"pull_request": 5, "action": action}), "{full:?}");
    }
}

#[test]
fn approving_a_merged_pr_reports_the_conflict() {
    let mut env = Env::new();
    env.mock("DELETE", &format!("{REPO}/pullrequests/5/approve"))
        .with_status(400)
        .with_body(r#"{"type":"error","error":{"message":"Pull request has already been merged"}}"#)
        .create();

    let out = env.bibu().args(["pr", "unapprove", "5"]).output().unwrap();

    assert_eq!(out.status.code(), Some(6));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("already been merged"));
}

// ---------------------------------------------------------------- merge & decline

#[test]
fn merge_without_yes_and_without_a_terminal_is_refused_with_no_request() {
    let mut env = Env::new();
    let get = env
        .mock("GET", &format!("{REPO}/pullrequests/5"))
        .expect(0)
        .create();
    let merge = env
        .mock("POST", &format!("{REPO}/pullrequests/5/merge"))
        .expect(0)
        .create();

    let out = env.bibu().args(["pr", "merge", "5"]).output().unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--yes"));
    get.assert();
    merge.assert();
}

#[test]
fn merge_with_yes_posts_an_empty_body_by_default() {
    let mut env = Env::new();
    let mut merged = pr_json(5, "t", "a", "b", account("J", "{j}"));
    merged["state"] = json!("MERGED");
    merged["merge_commit"] = json!({"hash": "deadbeef"});
    let mock = env
        .mock("POST", &format!("{REPO}/pullrequests/5/merge"))
        .match_body(Matcher::Json(json!({})))
        .with_body(merged.to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "merge", "5", "--yes"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v["state"], "MERGED");
    assert_eq!(v["merge_commit"], "deadbeef");
}

#[test]
fn merge_options_reach_the_body() {
    let mut env = Env::new();
    let mock = env
        .mock("POST", &format!("{REPO}/pullrequests/5/merge"))
        .match_body(Matcher::Json(
            json!({"merge_strategy": "squash", "message": "Squashed", "close_source_branch": true}),
        ))
        .with_body(pr_json(5, "t", "a", "b", account("J", "{j}")).to_string())
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "merge",
            "5",
            "-y",
            "--strategy",
            "squash",
            "-m",
            "Squashed",
            "--close-source-branch",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
}

#[test]
fn merge_conflict_exits_6() {
    let mut env = Env::new();
    env.mock("POST", &format!("{REPO}/pullrequests/5/merge"))
        .with_status(409)
        .with_body(r#"{"type":"error","error":{"message":"Merge conflicts"}}"#)
        .create();

    let out = env
        .bibu()
        .args(["pr", "merge", "5", "--yes"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(6));
    assert_eq!(stderr_json(&out)["error"]["code"], "conflict");
}

#[test]
fn merge_rejects_unknown_strategies_and_contradictory_flags() {
    let env = Env::new();
    for args in [
        vec!["pr", "merge", "5", "--yes", "--strategy", "yolo"],
        vec![
            "pr",
            "merge",
            "5",
            "--yes",
            "--close-source-branch",
            "--keep-source-branch",
        ],
    ] {
        assert_eq!(
            env.bibu().args(&args).output().unwrap().status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn decline_needs_yes_then_posts() {
    let mut env = Env::new();
    let refused = env.bibu().args(["pr", "decline", "5"]).output().unwrap();
    assert_eq!(refused.status.code(), Some(2));

    let mut declined = pr_json(5, "t", "a", "b", account("J", "{j}"));
    declined["state"] = json!("DECLINED");
    let mock = env.json("POST", &format!("{REPO}/pullrequests/5/decline"), declined);

    let out = env
        .bibu()
        .args(["pr", "decline", "5", "--yes"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    assert_eq!(stdout_json(&out)["state"], "DECLINED");
}

// --------------------------------------------------------------- cross-cutting

#[test]
fn every_pr_command_requires_a_login() {
    let env = Env::new();
    let out = env
        .bibu()
        .env_remove("BIBU_EMAIL")
        .env_remove("BIBU_TOKEN")
        .args(["pr", "list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stderr_json(&out)["error"]["code"], "auth_invalid");
}

#[test]
fn pr_commands_need_a_repository() {
    let env = Env::new();
    let out = env
        .bibu()
        .env_remove("BIBU_REPO")
        .args(["pr", "list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--repo"));
}

#[test]
fn the_repo_flag_overrides_the_environment() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", "/repositories/other/thing/pullrequests")
        .match_query(Matcher::Any)
        .with_body(page(vec![]).to_string())
        .create();

    let out = env
        .bibu()
        .args([
            "pr",
            "list",
            "--repo",
            "https://bitbucket.org/other/thing.git",
        ])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
}

#[test]
fn an_expired_token_mid_command_is_exit_3() {
    let mut env = Env::new();
    env.mock("GET", &format!("{REPO}/pullrequests"))
        .match_query(Matcher::Any)
        .with_status(401)
        .with_body(r#"{"type":"error","error":{"message":"Token expired"}}"#)
        .create();

    let out = env.bibu().args(["pr", "list"]).output().unwrap();

    assert_eq!(out.status.code(), Some(3));
    assert!(stderr_json(&out)["error"]["hint"]
        .as_str()
        .unwrap()
        .contains("bibu auth login"));
}

#[test]
fn missing_scope_is_exit_4_and_names_the_scope() {
    let mut env = Env::new();
    env.mock("POST", &format!("{REPO}/pullrequests/5/approve"))
        .with_status(403)
        .with_body(r#"{"type":"error","error":{"message":"Your credentials lack one or more required privilege scopes.","detail":{"required":["write:pullrequest:bitbucket"]}}}"#)
        .create();

    let out = env.bibu().args(["pr", "approve", "5"]).output().unwrap();

    assert_eq!(out.status.code(), Some(4));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("write:pullrequest:bitbucket"));
}
