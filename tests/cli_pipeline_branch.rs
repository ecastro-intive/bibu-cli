//! End-to-end tests for `bibu pipeline ...` and `bibu branch ...` against a mock Bitbucket API.

mod common;

use common::*;
use mockito::Matcher;
use serde_json::{json, Value};

const REPO_URL: &str = "/repositories/acme/api";
const UUID: &str = "8f3c2a10-1b2c-4d5e-9f00-0a1b2c3d4e5f";
const UUID_PATH: &str = "8f3c2a10-1b2c-4d5e-9f00-0a1b2c3d4e5f";

fn pipelines_url() -> String {
    format!("{REPO_URL}/pipelines")
}

fn pipeline_json(number: u64, branch: &str, result: Option<&str>) -> Value {
    let state = match result {
        Some(r) => json!({"name": "COMPLETED", "result": {"name": r}}),
        None => json!({"name": "IN_PROGRESS", "stage": {"name": "RUNNING"}}),
    };
    json!({
        "uuid": format!("{{{UUID}}}"), "build_number": number,
        "creator": account("Jane Doe", "{j}"),
        "target": {"ref_type": "branch", "ref_name": branch, "commit": {"hash": "abcdef1234567890"}},
        "trigger": {"name": "PUSH"}, "state": state,
        "created_on": "2026-09-30T10:00:00+00:00", "completed_on": "2026-09-30T10:01:23+00:00",
        "duration_in_seconds": 83,
    })
}

fn step_json(uuid: &str, name: &str, result: &str) -> Value {
    json!({"uuid": uuid, "name": name, "state": {"name": "COMPLETED", "result": {"name": result}}, "duration_in_seconds": 12})
}

fn steps_page() -> Value {
    page(vec![
        step_json("{s-1}", "build", "SUCCESSFUL"),
        step_json("{s-2}", "Test", "FAILED"),
    ])
}

// --------------------------------------------------------------- pipeline list

#[test]
fn pipeline_list_is_newest_first_with_status_and_url() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &pipelines_url())
        .match_query(query(&[("sort", "-created_on"), ("pagelen", "25")]))
        .with_body(
            page(vec![
                pipeline_json(42, "main", Some("FAILED")),
                pipeline_json(41, "main", None),
            ])
            .to_string(),
        )
        .create();

    let out = env.bibu().args(["pipeline", "list"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v[0]["build_number"], 42);
    assert_eq!(v[0]["status"], "failed");
    assert_eq!(v[0]["branch"], "main");
    assert_eq!(v[0]["commit"], "abcdef1234567890");
    assert_eq!(v[0]["trigger"], "PUSH");
    assert_eq!(v[0]["duration_seconds"], 83);
    assert_eq!(
        v[0]["url"],
        "https://bitbucket.org/acme/api/pipelines/results/42"
    );
    assert_eq!(v[1]["status"], "running");
}

#[test]
fn pipeline_list_branch_filter_goes_to_the_server_and_is_rechecked_locally() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &pipelines_url())
        .match_query(query(&[("target.branch", "main"), ("sort", "-created_on")]))
        .with_body(
            page(vec![
                pipeline_json(3, "develop", Some("SUCCESSFUL")),
                pipeline_json(2, "main", Some("SUCCESSFUL")),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pipeline", "list", "--branch", "main"])
        .output()
        .unwrap();

    mock.assert();
    let ids: Vec<u64> = stdout_json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["build_number"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![2]);
}

#[test]
fn pipeline_list_status_filter_applies_across_the_whole_result() {
    let mut env = Env::new();
    env.mock("GET", &pipelines_url())
        .match_query(Matcher::Any)
        .with_body(
            page(vec![
                pipeline_json(5, "main", Some("SUCCESSFUL")),
                pipeline_json(4, "main", Some("FAILED")),
                pipeline_json(3, "main", None),
                pipeline_json(2, "main", Some("FAILED")),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pipeline", "list", "--status", "failed"])
        .output()
        .unwrap();

    let ids: Vec<u64> = stdout_json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["build_number"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![4, 2]);
}

#[test]
fn pipeline_list_all_follows_next_links_and_bad_flags_exit_2() {
    let mut env = Env::new();
    let next = format!("{}{}?page=2", env.server.url(), pipelines_url());
    let first = env
        .mock("GET", &pipelines_url())
        .match_query(query(&[("pagelen", "100")]))
        .with_body(
            json!({"values": [pipeline_json(2, "main", Some("SUCCESSFUL"))], "next": next})
                .to_string(),
        )
        .create();
    let second = env
        .mock("GET", &pipelines_url())
        .match_query(query(&[("page", "2")]))
        .with_body(page(vec![pipeline_json(1, "main", Some("SUCCESSFUL"))]).to_string())
        .create();
    let out = env
        .bibu()
        .args(["pipeline", "list", "--all"])
        .output()
        .unwrap();
    first.assert();
    second.assert();
    assert_eq!(stdout_json(&out).as_array().unwrap().len(), 2);

    let env = Env::new();
    for args in [
        vec!["pipeline", "list", "--status", "bogus"],
        vec!["pipeline", "list", "--limit", "0"],
    ] {
        assert_eq!(
            env.bibu().args(&args).output().unwrap().status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

#[test]
fn pipelines_not_enabled_or_no_scope_are_reported() {
    let mut env = Env::new();
    env.mock("GET", &pipelines_url())
        .match_query(Matcher::Any)
        .with_status(403)
        .with_body(r#"{"type":"error","error":{"message":"Your credentials lack one or more required privilege scopes.","detail":{"required":["read:pipeline:bitbucket"]}}}"#)
        .create();
    let out = env.bibu().args(["pipeline", "list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read:pipeline:bitbucket"));
}

// --------------------------------------------------------------- pipeline view

#[test]
fn pipeline_view_by_uuid_returns_the_run_and_its_steps() {
    let mut env = Env::new();
    let run = env.json(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D", pipelines_url()),
        pipeline_json(42, "main", Some("FAILED")),
    );
    let steps = env
        .mock(
            "GET",
            &format!("{}/%7B{UUID_PATH}%7D/steps", pipelines_url()),
        )
        .match_query(query(&[("pagelen", "100")]))
        .with_body(steps_page().to_string())
        .create();

    let out = env
        .bibu()
        .args(["pipeline", "view", &format!("{{{UUID}}}")])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    run.assert();
    steps.assert();
    let v = stdout_json(&out);
    assert_eq!(v["build_number"], 42);
    assert_eq!(
        v["steps"][0],
        json!({"uuid": "{s-1}", "name": "build", "status": "successful", "started_on": null, "completed_on": null, "duration_seconds": 12})
    );
    assert_eq!(v["steps"][1]["status"], "failed");
}

#[test]
fn a_bare_uuid_gets_its_braces() {
    let mut env = Env::new();
    let run = env.json(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D", pipelines_url()),
        pipeline_json(42, "main", Some("SUCCESSFUL")),
    );
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps", pipelines_url()),
    )
    .match_query(Matcher::Any)
    .with_body(page(vec![]).to_string())
    .create();

    let out = env
        .bibu()
        .args(["pipeline", "view", UUID])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    run.assert();
}

#[test]
fn pipeline_view_by_build_number_uses_the_direct_lookup_first() {
    let mut env = Env::new();
    let direct = env.json(
        "GET",
        &format!("{}/42", pipelines_url()),
        pipeline_json(42, "main", Some("SUCCESSFUL")),
    );
    let scan = env
        .mock("GET", &pipelines_url())
        .match_query(Matcher::Any)
        .expect(0)
        .create();
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps", pipelines_url()),
    )
    .match_query(Matcher::Any)
    .with_body(steps_page().to_string())
    .create();

    let out = env
        .bibu()
        .args(["pipeline", "view", "42"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    direct.assert();
    scan.assert();
}

#[test]
fn pipeline_view_falls_back_to_scanning_when_the_number_is_not_a_direct_key() {
    let mut env = Env::new();
    let miss = env
        .mock("GET", &format!("{}/42", pipelines_url()))
        .with_status(404)
        .with_body(r#"{"type":"error","error":{"message":"not found"}}"#)
        .create();
    let scan = env
        .mock("GET", &pipelines_url())
        .match_query(query(&[("sort", "-created_on")]))
        .with_body(
            page(vec![
                pipeline_json(43, "main", Some("SUCCESSFUL")),
                pipeline_json(42, "main", Some("FAILED")),
            ])
            .to_string(),
        )
        .create();
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps", pipelines_url()),
    )
    .match_query(Matcher::Any)
    .with_body(steps_page().to_string())
    .create();

    let out = env
        .bibu()
        .args(["pipeline", "view", "42"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    miss.assert();
    scan.assert();
    assert_eq!(stdout_json(&out)["status"], "failed");
}

#[test]
fn pipeline_view_unknown_number_is_not_found() {
    let mut env = Env::new();
    env.mock("GET", &format!("{}/9", pipelines_url()))
        .with_status(404)
        .with_body("{}")
        .create();
    env.mock("GET", &pipelines_url())
        .match_query(Matcher::Any)
        .with_body(page(vec![pipeline_json(1, "main", Some("SUCCESSFUL"))]).to_string())
        .create();

    let out = env.bibu().args(["pipeline", "view", "9"]).output().unwrap();

    assert_eq!(out.status.code(), Some(5));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("build number 9"));
}

#[test]
fn pipeline_commands_reject_malformed_ids_before_any_request() {
    let mut env = Env::new();
    let never = env.server.mock("GET", Matcher::Any).expect(0).create();
    for args in [
        vec!["pipeline", "view", "abc"],
        vec!["pipeline", "logs", "{nope}"],
        vec!["pipeline", "view", "-5"],
    ] {
        assert_eq!(
            env.bibu().args(&args).output().unwrap().status.code(),
            Some(2),
            "{args:?}"
        );
    }
    never.assert();
}

// --------------------------------------------------------------- pipeline logs

fn logs_env(env: &mut Env) {
    env.json(
        "GET",
        &format!("{}/7", pipelines_url()),
        pipeline_json(7, "main", Some("FAILED")),
    );
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps", pipelines_url()),
    )
    .match_query(Matcher::Any)
    .with_body(steps_page().to_string())
    .create();
}

#[test]
fn logs_print_every_step_under_a_header_as_raw_text_even_when_piped() {
    let mut env = Env::new();
    logs_env(&mut env);
    let build = env
        .mock(
            "GET",
            &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-1%7D/log", pipelines_url()),
        )
        .with_body("compiling\ndone\n")
        .create();
    let test = env
        .mock(
            "GET",
            &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-2%7D/log", pipelines_url()),
        )
        .with_body("1 failed\n")
        .create();

    let out = env.bibu().args(["pipeline", "logs", "7"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    build.assert();
    test.assert();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "==> build (successful) <==\ncompiling\ndone\n\n==> Test (failed) <==\n1 failed\n"
    );
}

#[test]
fn logs_for_one_step_are_bare_and_the_step_can_be_named_numbered_or_uuid() {
    for selector in ["test", "2", "{s-2}"] {
        let mut env = Env::new();
        logs_env(&mut env);
        let only = env
            .mock(
                "GET",
                &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-2%7D/log", pipelines_url()),
            )
            .with_body("1 failed\n")
            .create();
        let never = env
            .mock(
                "GET",
                &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-1%7D/log", pipelines_url()),
            )
            .expect(0)
            .create();

        let out = env
            .bibu()
            .args(["pipeline", "logs", "7", "--step", selector])
            .output()
            .unwrap();

        assert!(
            out.status.success(),
            "{selector}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        only.assert();
        never.assert();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "1 failed\n",
            "{selector}"
        );
    }
}

#[test]
fn logs_with_json_wrap_each_step() {
    let mut env = Env::new();
    logs_env(&mut env);
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-1%7D/log", pipelines_url()),
    )
    .with_body("ok\n")
    .create();
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-2%7D/log", pipelines_url()),
    )
    .with_body("bad\n")
    .create();

    let out = env
        .bibu()
        .args(["pipeline", "logs", "7", "--json"])
        .output()
        .unwrap();

    let v = stdout_json(&out);
    assert_eq!(v["pipeline"], 7);
    assert_eq!(v["steps"][0]["name"], "build");
    assert_eq!(v["steps"][0]["log"], "ok\n");
    assert_eq!(v["steps"][1]["status"], "failed");
}

#[test]
fn a_step_without_a_log_yet_is_reported_not_fatal() {
    let mut env = Env::new();
    logs_env(&mut env);
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-1%7D/log", pipelines_url()),
    )
    .with_body("ok\n")
    .create();
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-2%7D/log", pipelines_url()),
    )
    .with_status(404)
    .with_body("{}")
    .create();

    let table = env.bibu().args(["pipeline", "logs", "7"]).output().unwrap();
    assert!(table.status.success());
    assert!(String::from_utf8_lossy(&table.stdout)
        .contains("==> Test (failed) <==\n(no log available yet)"));

    let json_out = env
        .bibu()
        .args(["pipeline", "logs", "7", "--json"])
        .output()
        .unwrap();
    assert_eq!(stdout_json(&json_out)["steps"][1]["log"], Value::Null);
}

#[test]
fn logs_follow_the_redirect_to_long_term_storage() {
    let mut env = Env::new();
    logs_env(&mut env);
    let target = format!("{}/stored/log-1", env.server.url());
    env.mock(
        "GET",
        &format!("{}/%7B{UUID_PATH}%7D/steps/%7Bs-1%7D/log", pipelines_url()),
    )
    .with_status(307)
    .with_header("location", &target)
    .create();
    let stored = env
        .mock("GET", "/stored/log-1")
        .with_body("archived log\n")
        .create();

    let out = env
        .bibu()
        .args(["pipeline", "logs", "7", "--step", "1"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stored.assert();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "archived log\n");
}

#[test]
fn logs_unknown_or_out_of_range_steps_are_errors() {
    for (selector, code, needle) in [("9", 2, "out of range"), ("deploy", 5, "steps are")] {
        let mut env = Env::new();
        logs_env(&mut env);
        let out = env
            .bibu()
            .args(["pipeline", "logs", "7", "--step", selector])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(code), "{selector}");
        assert!(
            stderr_json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains(needle),
            "{selector}"
        );
    }
}

// --------------------------------------------------------------------- branches

fn repo_body(default: &str) -> Value {
    json!({"full_name": "acme/api", "mainbranch": {"type": "branch", "name": default}})
}

fn branch_json(name: &str, hash: &str) -> Value {
    json!({"name": name, "target": {"hash": hash, "date": "2026-09-30T10:00:00+00:00", "message": "Do a thing\n\nbody", "author": {"raw": "Jane Doe <j@x.io>"}}})
}

fn branches_url() -> String {
    format!("{REPO_URL}/refs/branches")
}

#[test]
fn branch_list_marks_the_default_branch() {
    let mut env = Env::new();
    let list = env
        .mock("GET", &branches_url())
        .match_query(query(&[("sort", "-target.date"), ("pagelen", "25")]))
        .with_body(
            page(vec![
                branch_json("feature/x", "1111111aaaa"),
                branch_json("main", "2222222bbbb"),
            ])
            .to_string(),
        )
        .create();
    let repo = env.json("GET", REPO_URL, repo_body("main"));

    let out = env.bibu().args(["branch", "list"]).output().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    list.assert();
    repo.assert();
    let v = stdout_json(&out);
    assert_eq!(v[0]["name"], "feature/x");
    assert_eq!(v[0]["default"], false);
    assert_eq!(v[1]["default"], true);
    assert_eq!(v[0]["author"], "Jane Doe");
    assert_eq!(v[0]["message"], "Do a thing");
    assert_eq!(v[0]["hash"], "1111111aaaa");
}

#[test]
fn branch_list_name_filter_uses_a_quoted_q_expression() {
    let mut env = Env::new();
    let list = env
        .mock("GET", &branches_url())
        .match_query(query(&[("q", r#"name ~ "fe\"at""#)]))
        .with_body(
            page(vec![
                branch_json("fe\"at-1", "1111111aaaa"),
                branch_json("other", "2222222bbbb"),
            ])
            .to_string(),
        )
        .create();
    env.json("GET", REPO_URL, repo_body("main"));

    let out = env
        .bibu()
        .args(["branch", "list", "--name", "fe\"at"])
        .output()
        .unwrap();

    list.assert();
    let v = stdout_json(&out);
    let names: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["fe\"at-1"]);
}

#[test]
fn branch_create_defaults_to_the_repos_default_branch() {
    let mut env = Env::new();
    let repo = env.json("GET", REPO_URL, repo_body("main"));
    let tip = env.json(
        "GET",
        &format!("{}/main", branches_url()),
        branch_json("main", "2222222bbbb"),
    );
    let create = env
        .mock("POST", &branches_url())
        .match_body(Matcher::Json(
            json!({"name": "feature/new", "target": {"hash": "2222222bbbb"}}),
        ))
        .with_status(201)
        .with_body(branch_json("feature/new", "2222222bbbb").to_string())
        .create();

    let out = env
        .bibu()
        .args(["branch", "create", "feature/new"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    repo.assert();
    tip.assert();
    create.assert();
    assert_eq!(stdout_json(&out)["name"], "feature/new");
}

#[test]
fn branch_create_from_a_branch_with_a_slash_encodes_nothing_it_should_not() {
    let mut env = Env::new();
    let repo = env.mock("GET", REPO_URL).expect(0).create();
    env.json(
        "GET",
        &format!("{}/feature/base", branches_url()),
        branch_json("feature/base", "3333333cccc"),
    );
    let create = env
        .mock("POST", &branches_url())
        .match_body(Matcher::Json(
            json!({"name": "feature/copy", "target": {"hash": "3333333cccc"}}),
        ))
        .with_status(201)
        .with_body(branch_json("feature/copy", "3333333cccc").to_string())
        .create();

    let out = env
        .bibu()
        .args(["branch", "create", "feature/copy", "--from", "feature/base"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    repo.assert();
    create.assert();
}

#[test]
fn branch_create_from_a_commit_hash_when_no_branch_has_that_name() {
    let mut env = Env::new();
    env.mock("GET", &format!("{}/abc1234", branches_url()))
        .with_status(404)
        .with_body("{}")
        .create();
    let create = env
        .mock("POST", &branches_url())
        .match_body(Matcher::Json(
            json!({"name": "hotfix", "target": {"hash": "abc1234"}}),
        ))
        .with_status(201)
        .with_body(branch_json("hotfix", "abc1234def").to_string())
        .create();

    let out = env
        .bibu()
        .args(["branch", "create", "hotfix", "--from", "abc1234"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    create.assert();
}

#[test]
fn branch_create_errors_send_nothing_they_should_not() {
    // unknown, non-hash start point
    let mut env = Env::new();
    env.mock("GET", &format!("{}/nope", branches_url()))
        .with_status(404)
        .with_body("{}")
        .create();
    let never = env.mock("POST", &branches_url()).expect(0).create();
    let out = env
        .bibu()
        .args(["branch", "create", "x", "--from", "nope"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
    never.assert();

    // invalid names are refused locally, without any request
    let mut env = Env::new();
    let any = env.server.mock("GET", Matcher::Any).expect(0).create();
    for name in ["a b", "-x"] {
        assert_eq!(
            env.bibu()
                .args(["branch", "create", name])
                .output()
                .unwrap()
                .status
                .code(),
            Some(2),
            "{name}"
        );
    }
    any.assert();

    // the branch already exists
    let mut env = Env::new();
    env.json("GET", REPO_URL, repo_body("main"));
    env.json(
        "GET",
        &format!("{}/main", branches_url()),
        branch_json("main", "2222222bbbb"),
    );
    env.mock("POST", &branches_url())
        .with_status(400)
        .with_body(r#"{"type":"error","error":{"message":"Branch \"dup\" already exists"}}"#)
        .create();
    let out = env
        .bibu()
        .args(["branch", "create", "dup"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(6));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("already exists"));
}

#[test]
fn branch_delete_needs_yes_and_never_touches_the_default_branch() {
    // no --yes, no terminal
    let mut env = Env::new();
    env.json("GET", REPO_URL, repo_body("main"));
    let never = env.server.mock("DELETE", Matcher::Any).expect(0).create();
    let out = env
        .bibu()
        .args(["branch", "delete", "feature/x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--yes"));
    never.assert();

    // default branch is refused even with --yes
    let mut env = Env::new();
    env.json("GET", REPO_URL, repo_body("main"));
    let never = env.server.mock("DELETE", Matcher::Any).expect(0).create();
    let out = env
        .bibu()
        .args(["branch", "delete", "main", "--yes"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(6));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("default branch"));
    never.assert();
}

#[test]
fn branch_delete_with_yes_encodes_awkward_names() {
    let mut env = Env::new();
    env.json("GET", REPO_URL, repo_body("main"));
    let delete = env
        .mock("DELETE", &format!("{}/tmp/odd%231%25", branches_url()))
        .with_status(204)
        .create();

    let out = env
        .bibu()
        .args(["branch", "delete", "tmp/odd#1%", "--yes"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    delete.assert();
    assert_eq!(
        stdout_json(&out),
        json!({"branch": "tmp/odd#1%", "action": "deleted"})
    );
}

#[test]
fn branch_delete_of_a_missing_branch_is_not_found() {
    let mut env = Env::new();
    env.json("GET", REPO_URL, repo_body("main"));
    env.mock("DELETE", &format!("{}/ghost", branches_url()))
        .with_status(404)
        .with_body(r#"{"type":"error","error":{"message":"Branch not found"}}"#)
        .create();
    let out = env
        .bibu()
        .args(["branch", "delete", "ghost", "--yes"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
}
