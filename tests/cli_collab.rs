//! End-to-end tests for `bibu pr comment | reviewers | task` and `bibu member`.
//!
//! Every mock asserts the exact method, path, query, JSON body and `Authorization` header.

mod common;

use common::*;
use mockito::Matcher;
use serde_json::{json, Value};

const PR: &str = "/repositories/acme/api/pullrequests/12";

fn comment_json(id: u64, text: &str) -> Value {
    json!({
        "id": id, "content": {"raw": text, "markup": "markdown"},
        "user": account("Jane Doe", "{j}"),
        "created_on": "2026-09-30T10:00:00+00:00", "updated_on": "2026-09-30T10:00:00+00:00",
        "deleted": false, "pending": false,
        "links": {"html": {"href": format!("https://bitbucket.org/acme/api/pull-requests/12/_/diff#comment-{id}")}},
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

fn comments_url() -> String {
    format!("{PR}/comments")
}

// ------------------------------------------------------------------- comment list

#[test]
fn comment_list_returns_rows_with_thread_fields_and_hides_deleted() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &comments_url())
        .match_query(query(&[("pagelen", "25")]))
        .with_body(
            page(vec![
                with(comment_json(1, "general"), json!({})),
                with(comment_json(2, "inline"), json!({"inline": {"path": "src/a.rs", "start_to": 3, "to": 5}, "resolution": {"user": account("Bob Ray", "{b}")}})),
                with(comment_json(3, "reply"), json!({"parent": {"id": 2}})),
                with(comment_json(4, "gone"), json!({"deleted": true})),
            ])
            .to_string(),
        )
        .create();

    let out = env
        .bibu()
        .args(["pr", "comment", "list", "12"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    let v = stdout_json(&out);
    let ids: Vec<u64> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![1, 2, 3]);
    assert_eq!(v[0]["inline"], Value::Null);
    assert_eq!(
        v[1]["inline"],
        json!({"path": "src/a.rs", "line": 3, "end_line": 5, "side": "new"})
    );
    assert_eq!(v[1]["resolved"], true);
    assert_eq!(v[1]["resolved_by"]["display_name"], "Bob Ray");
    assert_eq!(v[2]["parent_id"], 2);
    assert_eq!(
        v[0]["url"],
        "https://bitbucket.org/acme/api/pull-requests/12/_/diff#comment-1"
    );
}

fn mixed_thread() -> Value {
    page(vec![
        comment_json(1, "general"),
        with(
            comment_json(2, "open inline"),
            json!({"inline": {"path": "a.rs", "to": 1}}),
        ),
        with(comment_json(3, "reply"), json!({"parent": {"id": 2}})),
        with(
            comment_json(4, "done inline"),
            json!({"inline": {"path": "b.rs", "to": 2}, "resolution": {}}),
        ),
        with(comment_json(5, "gone"), json!({"deleted": true})),
    ])
}

fn listed_ids(env: &mut Env, args: &[&str]) -> Vec<u64> {
    env.mock("GET", &comments_url())
        .match_query(Matcher::Any)
        .with_body(mixed_thread().to_string())
        .create();
    let mut full = vec!["pr", "comment", "list", "12"];
    full.extend_from_slice(args);
    let out = env.bibu().args(&full).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout_json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_u64().unwrap())
        .collect()
}

#[test]
fn comment_list_filters() {
    let cases: [(&[&str], Vec<u64>); 5] = [
        (&[], vec![1, 2, 3, 4]),
        (&["--unresolved"], vec![1, 2, 3]),
        (&["--inline"], vec![2, 3, 4]),
        (&["--file", "a.rs"], vec![2, 3]),
        (&["--include-deleted"], vec![1, 2, 3, 4, 5]),
    ];
    for (args, expected) in cases {
        let mut env = Env::new();
        assert_eq!(listed_ids(&mut env, args), expected, "{args:?}");
    }
}

#[test]
fn comment_list_all_follows_pagination() {
    let mut env = Env::new();
    let next = format!("{}{}?page=2", env.server.url(), comments_url());
    let first = env
        .mock("GET", &comments_url())
        .match_query(query(&[("pagelen", "100")]))
        .with_body(json!({"values": [comment_json(1, "a")], "next": next}).to_string())
        .create();
    let second = env
        .mock("GET", &comments_url())
        .match_query(query(&[("page", "2")]))
        .with_body(page(vec![comment_json(2, "b")]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "comment", "list", "12", "--all"])
        .output()
        .unwrap();

    first.assert();
    second.assert();
    assert_eq!(stdout_json(&out).as_array().unwrap().len(), 2);
}

// -------------------------------------------------------------------- comment add

fn add_with_body(args: &[&str], body: Value) -> Value {
    let mut env = Env::new();
    let mock = env
        .mock("POST", &comments_url())
        .match_body(Matcher::Json(body))
        .with_status(201)
        .with_body(comment_json(99, "created").to_string())
        .create();
    let mut full = vec!["pr", "comment", "add", "12"];
    full.extend_from_slice(args);
    let out = env.bibu().args(&full).output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    stdout_json(&out)
}

#[test]
fn comment_add_general() {
    let v = add_with_body(&["Looks good"], json!({"content": {"raw": "Looks good"}}));
    assert_eq!(v["id"], 99);
}

#[test]
fn comment_add_anchor_variants_send_the_right_inline_object() {
    let cases: [(&[&str], Value); 6] = [
        (&["--file", "src/a.rs", "hi"], json!({"path": "src/a.rs"})),
        (
            &["--file", "src/a.rs", "--line", "7", "hi"],
            json!({"path": "src/a.rs", "to": 7}),
        ),
        (
            &["-f", "./src/a.rs", "-l", "7", "hi"],
            json!({"path": "src/a.rs", "to": 7}),
        ),
        (
            &["--file", "src/a.rs", "--line", "4", "--end-line", "6", "hi"],
            json!({"path": "src/a.rs", "start_to": 4, "to": 6}),
        ),
        (
            &["--file", "src/a.rs", "--line", "2", "--old-side", "hi"],
            json!({"path": "src/a.rs", "from": 2}),
        ),
        (
            &[
                "--file",
                "src/a.rs",
                "--line",
                "2",
                "--end-line",
                "3",
                "--old-side",
                "hi",
            ],
            json!({"path": "src/a.rs", "start_from": 2, "from": 3}),
        ),
    ];
    for (args, inline) in cases {
        add_with_body(args, json!({"content": {"raw": "hi"}, "inline": inline}));
    }
}

#[test]
fn comment_add_reads_text_from_stdin_when_omitted_or_dash() {
    for args in [
        vec!["pr", "comment", "add", "12"],
        vec!["pr", "comment", "add", "12", "-"],
    ] {
        let mut env = Env::new();
        let mock = env
            .mock("POST", &comments_url())
            .match_body(Matcher::Json(
                json!({"content": {"raw": "line one\nline two"}}),
            ))
            .with_status(201)
            .with_body(comment_json(99, "x").to_string())
            .create();

        let out = env
            .bibu()
            .args(&args)
            .write_stdin("line one\nline two\n\n")
            .output()
            .unwrap();

        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        mock.assert();
    }
}

#[test]
fn comment_add_rejects_bad_input_before_any_request() {
    let cases: [(&[&str], &str); 6] = [
        (
            &["pr", "comment", "add", "12", "--line", "3", "hi"],
            "--file",
        ),
        (
            &[
                "pr",
                "comment",
                "add",
                "12",
                "--file",
                "a.rs",
                "--end-line",
                "3",
                "hi",
            ],
            "--line",
        ),
        (
            &[
                "pr",
                "comment",
                "add",
                "12",
                "--file",
                "a.rs",
                "--old-side",
                "hi",
            ],
            "--line",
        ),
        (
            &[
                "pr",
                "comment",
                "add",
                "12",
                "--file",
                "a.rs",
                "--line",
                "5",
                "--end-line",
                "4",
                "hi",
            ],
            "must not be before",
        ),
        (
            &[
                "pr", "comment", "add", "12", "--file", "a.rs", "--line", "0", "hi",
            ],
            "0",
        ),
        (&["pr", "comment", "add", "12", "   "], "empty"),
    ];
    for (args, needle) in cases {
        let mut env = Env::new();
        let never = env.mock("POST", &comments_url()).expect(0).create();

        let out = env.bibu().args(args).write_stdin("").output().unwrap();

        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        never.assert();
    }
}

#[test]
fn comment_add_on_a_missing_pr_is_not_found() {
    let mut env = Env::new();
    env.mock("POST", &comments_url())
        .with_status(404)
        .with_body(r#"{"type":"error","error":{"message":"No pull request found"}}"#)
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "add", "12", "hi"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
}

// ------------------------------------------------------- reply/edit/delete/resolve

#[test]
fn comment_reply_carries_the_parent() {
    let mut env = Env::new();
    let mock = env
        .mock("POST", &comments_url())
        .match_body(Matcher::Json(
            json!({"content": {"raw": "agreed"}, "parent": {"id": 5}}),
        ))
        .with_status(201)
        .with_body(with(comment_json(6, "agreed"), json!({"parent": {"id": 5}})).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "comment", "reply", "12", "5", "agreed"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    assert_eq!(stdout_json(&out)["parent_id"], 5);
}

#[test]
fn comment_edit_puts_new_content_only() {
    let mut env = Env::new();
    let mock = env
        .mock("PUT", &format!("{}/5", comments_url()))
        .match_body(Matcher::Json(json!({"content": {"raw": "fixed typo"}})))
        .with_body(comment_json(5, "fixed typo").to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "comment", "edit", "12", "5", "fixed typo"])
        .output()
        .unwrap();

    mock.assert();
    assert_eq!(stdout_json(&out)["content"], "fixed typo");
}

#[test]
fn comment_delete_needs_yes_without_a_terminal() {
    let mut env = Env::new();
    let never = env
        .mock("DELETE", &format!("{}/5", comments_url()))
        .expect(0)
        .create();
    let refused = env
        .bibu()
        .args(["pr", "comment", "delete", "12", "5"])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert!(stderr_json(&refused)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("--yes"));
    never.assert();

    let mut env = Env::new();
    let mock = env
        .mock("DELETE", &format!("{}/5", comments_url()))
        .with_status(204)
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "delete", "12", "5", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    mock.assert();
    assert_eq!(
        stdout_json(&out),
        json!({"pull_request": 12, "comment": 5, "action": "deleted"})
    );
}

#[test]
fn comment_resolve_and_reopen_use_post_and_delete_on_resolve() {
    let mut env = Env::new();
    let resolve = env
        .mock("POST", &format!("{}/5/resolve", comments_url()))
        .with_body("{}")
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "resolve", "12", "5"])
        .output()
        .unwrap();
    resolve.assert();
    assert_eq!(
        stdout_json(&out),
        json!({"pull_request": 12, "comment": 5, "action": "resolved"})
    );

    let mut env = Env::new();
    let reopen = env
        .mock("DELETE", &format!("{}/5/resolve", comments_url()))
        .with_status(204)
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "reopen", "12", "5"])
        .output()
        .unwrap();
    reopen.assert();
    assert_eq!(
        stdout_json(&out),
        json!({"pull_request": 12, "comment": 5, "action": "reopened"})
    );
}

#[test]
fn resolving_twice_is_a_conflict_and_reopening_twice_is_not_found() {
    let mut env = Env::new();
    env.mock("POST", &format!("{}/5/resolve", comments_url()))
        .with_status(409)
        .with_body(r#"{"type":"error","error":{"message":"Comment has already been resolved."}}"#)
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "resolve", "12", "5"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(6));
    assert_eq!(
        stderr_json(&out)["error"]["message"],
        "Comment has already been resolved."
    );

    let mut env = Env::new();
    env.mock("DELETE", &format!("{}/5/resolve", comments_url()))
        .with_status(404)
        .with_body(r#"{"type":"error","error":{"message":"No PullRequestCommentResolution matches the given query."}}"#)
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "reopen", "12", "5"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
}

// -------------------------------------------------------------------- reviewers

fn pr_with_reviewers(reviewers: &[(&str, &str)]) -> Value {
    json!({
        "id": 12, "title": "T", "state": "OPEN",
        "reviewers": reviewers.iter().map(|(n, u)| account(n, u)).collect::<Vec<_>>(),
        "participants": reviewers.iter().map(|(n, u)| json!({"user": account(n, u), "role": "REVIEWER", "approved": false, "state": null})).collect::<Vec<_>>(),
    })
}

fn members_mock(env: &mut Env) -> mockito::Mock {
    env.mock("GET", "/workspaces/acme/members")
        .match_query(query(&[("pagelen", "100")]))
        .with_body(
            page(vec![
                json!({"user": account("Jane Doe", "{j}")}),
                json!({"user": account("Bob Ray", "{b}")}),
                json!({"user": account("Cy Lee", "{c}")}),
            ])
            .to_string(),
        )
        .create()
}

#[test]
fn reviewers_list_shows_reviewer_participants_only() {
    let mut env = Env::new();
    let mut body = pr_with_reviewers(&[("Bob Ray", "{b}")]);
    body["participants"].as_array_mut().unwrap().push(json!({"user": account("Ed Poe", "{e}"), "role": "PARTICIPANT", "approved": false, "state": null}));
    body["participants"][0]["state"] = json!("changes_requested");
    let mock = env.json("GET", PR, body);

    let out = env
        .bibu()
        .args(["pr", "reviewers", "list", "12"])
        .output()
        .unwrap();

    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v["pull_request"], 12);
    assert_eq!(v["reviewers"].as_array().unwrap().len(), 1);
    assert_eq!(v["reviewers"][0]["user"]["display_name"], "Bob Ray");
    assert_eq!(v["reviewers"][0]["state"], "changes_requested");
}

#[test]
fn reviewers_add_keeps_existing_and_sends_the_merged_list() {
    let mut env = Env::new();
    // Bitbucket replaces the whole list on update, so the body must contain Bob AND Cy.
    let before = env
        .mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}")]).to_string())
        .create();
    let members = members_mock(&mut env);
    let put = env
        .mock("PUT", PR)
        .match_body(Matcher::Json(
            json!({"reviewers": [{"uuid": "{b}"}, {"uuid": "{c}"}]}),
        ))
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}"), ("Cy Lee", "{c}")]).to_string())
        .create();
    let after = env
        .mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}"), ("Cy Lee", "{c}")]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "add", "12", "Cy Lee"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    before.assert();
    members.assert();
    put.assert();
    after.assert();
    let v = stdout_json(&out);
    let names: Vec<&str> = v["reviewers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["user"]["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Bob Ray", "Cy Lee"]);
}

#[test]
fn reviewers_add_with_a_uuid_or_me_skips_the_member_lookup() {
    let mut env = Env::new();
    let members = env
        .mock("GET", "/workspaces/acme/members")
        .match_query(Matcher::Any)
        .expect(0)
        .create();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[]).to_string())
        .expect(2)
        .create();
    let user = env.me();
    let put = env
        .mock("PUT", PR)
        .match_body(Matcher::Json(
            json!({"reviewers": [{"uuid": "{zzz}"}, {"uuid": "{me}"}]}),
        ))
        .with_body(pr_with_reviewers(&[]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "add", "12", "{zzz}", "me"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    members.assert();
    user.assert();
    put.assert();
}

#[test]
fn reviewers_add_is_idempotent_for_people_already_reviewing() {
    let mut env = Env::new();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}")]).to_string())
        .expect(2)
        .create();
    members_mock(&mut env);
    let put = env
        .mock("PUT", PR)
        .match_body(Matcher::Json(json!({"reviewers": [{"uuid": "{b}"}]})))
        .with_body(pr_with_reviewers(&[]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "add", "12", "Bob Ray"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    put.assert();
}

#[test]
fn reviewers_add_ambiguous_or_unknown_names_send_nothing() {
    for (name, code, needle) in [
        ("o", 2, "matches"),
        ("Zed Nobody", 5, "no workspace member"),
    ] {
        let mut env = Env::new();
        env.mock("GET", PR)
            .with_body(pr_with_reviewers(&[]).to_string())
            .create();
        members_mock(&mut env);
        let never = env.mock("PUT", PR).expect(0).create();

        let out = env
            .bibu()
            .args(["pr", "reviewers", "add", "12", name])
            .output()
            .unwrap();

        assert_eq!(
            out.status.code(),
            Some(code),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            stderr_json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains(needle),
            "{name}"
        );
        never.assert();
    }
}

#[test]
fn reviewers_add_the_author_is_reported_as_a_conflict() {
    let mut env = Env::new();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[]).to_string())
        .create();
    members_mock(&mut env);
    env.mock("PUT", PR)
        .with_status(400)
        .with_body(r#"{"type":"error","error":{"message":"reviewers: Jane Doe is the author and cannot be included as a reviewer."}}"#)
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "add", "12", "Jane Doe"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(6));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("is the author"));
}

#[test]
fn reviewers_remove_sends_the_list_without_them() {
    let mut env = Env::new();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}"), ("Cy Lee", "{c}")]).to_string())
        .expect(2)
        .create();
    let put = env
        .mock("PUT", PR)
        .match_body(Matcher::Json(json!({"reviewers": [{"uuid": "{c}"}]})))
        .with_body(pr_with_reviewers(&[]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "remove", "12", "Bob Ray"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    put.assert();
}

#[test]
fn reviewers_remove_the_last_one_sends_an_empty_list() {
    let mut env = Env::new();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}")]).to_string())
        .expect(2)
        .create();
    let put = env
        .mock("PUT", PR)
        .match_body(Matcher::Json(json!({"reviewers": []})))
        .with_body(pr_with_reviewers(&[]).to_string())
        .create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "remove", "12", "bob"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    put.assert();
}

#[test]
fn reviewers_remove_someone_who_is_not_a_reviewer_is_not_found() {
    let mut env = Env::new();
    env.mock("GET", PR)
        .with_body(pr_with_reviewers(&[("Bob Ray", "{b}")]).to_string())
        .create();
    let never = env.mock("PUT", PR).expect(0).create();

    let out = env
        .bibu()
        .args(["pr", "reviewers", "remove", "12", "Cy Lee"])
        .output()
        .unwrap();

    assert_eq!(out.status.code(), Some(5));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("current reviewer"));
    never.assert();
}

// ------------------------------------------------------------------------ tasks

fn task_json(id: u64, text: &str, state: &str) -> Value {
    json!({
        "id": id, "state": state, "content": {"raw": text},
        "creator": account("Jane Doe", "{j}"), "created_on": "2026-09-30T10:00:00+00:00",
    })
}

#[test]
fn task_list_and_unresolved_filter() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", &format!("{PR}/tasks"))
        .match_query(query(&[("pagelen", "25")]))
        .with_body(
            page(vec![
                with(
                    task_json(1, "open one", "UNRESOLVED"),
                    json!({"comment": {"id": 9}}),
                ),
                task_json(2, "done one", "RESOLVED"),
            ])
            .to_string(),
        )
        .create();
    let out = env
        .bibu()
        .args(["pr", "task", "list", "12"])
        .output()
        .unwrap();
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v.as_array().unwrap().len(), 2);
    assert_eq!(v[0]["comment_id"], 9);
    assert_eq!(v[1]["state"], "RESOLVED");

    let mut env = Env::new();
    env.mock("GET", &format!("{PR}/tasks"))
        .match_query(Matcher::Any)
        .with_body(
            page(vec![
                task_json(1, "open one", "UNRESOLVED"),
                task_json(2, "done one", "RESOLVED"),
            ])
            .to_string(),
        )
        .create();
    let out = env
        .bibu()
        .args(["pr", "task", "list", "12", "--unresolved"])
        .output()
        .unwrap();
    let ids: Vec<u64> = stdout_json(&out)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, vec![1]);
}

#[test]
fn task_add_standalone_and_attached() {
    let mut env = Env::new();
    let mock = env
        .mock("POST", &format!("{PR}/tasks"))
        .match_body(Matcher::Json(json!({"content": {"raw": "write tests"}})))
        .with_status(201)
        .with_body(task_json(7, "write tests", "UNRESOLVED").to_string())
        .create();
    let out = env
        .bibu()
        .args(["pr", "task", "add", "12", "write tests"])
        .output()
        .unwrap();
    mock.assert();
    assert_eq!(stdout_json(&out)["id"], 7);

    let mut env = Env::new();
    let mock = env
        .mock("POST", &format!("{PR}/tasks"))
        .match_body(Matcher::Json(
            json!({"content": {"raw": "answer"}, "comment": {"id": 9}}),
        ))
        .with_status(201)
        .with_body(
            with(
                task_json(8, "answer", "UNRESOLVED"),
                json!({"comment": {"id": 9}}),
            )
            .to_string(),
        )
        .create();
    let out = env
        .bibu()
        .args(["pr", "task", "add", "12", "answer", "--comment", "9"])
        .output()
        .unwrap();
    mock.assert();
    assert_eq!(stdout_json(&out)["comment_id"], 9);
}

#[test]
fn task_add_reads_stdin_and_rejects_empty_text() {
    let mut env = Env::new();
    let mock = env
        .mock("POST", &format!("{PR}/tasks"))
        .match_body(Matcher::Json(json!({"content": {"raw": "from stdin"}})))
        .with_status(201)
        .with_body(task_json(7, "from stdin", "UNRESOLVED").to_string())
        .create();
    env.bibu()
        .args(["pr", "task", "add", "12"])
        .write_stdin("from stdin\n")
        .assert()
        .success();
    mock.assert();

    let env = Env::new();
    let out = env
        .bibu()
        .args(["pr", "task", "add", "12"])
        .write_stdin("\n")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn task_resolve_and_reopen_send_the_state_only() {
    for (verb, state) in [("resolve", "RESOLVED"), ("reopen", "UNRESOLVED")] {
        let mut env = Env::new();
        let mock = env
            .mock("PUT", &format!("{PR}/tasks/7"))
            .match_body(Matcher::Json(json!({"state": state})))
            .with_body(task_json(7, "t", state).to_string())
            .create();

        let out = env
            .bibu()
            .args(["pr", "task", verb, "12", "7"])
            .output()
            .unwrap();

        assert!(
            out.status.success(),
            "{verb}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        mock.assert();
        assert_eq!(stdout_json(&out)["state"], state);
    }
}

// ----------------------------------------------------------------------- members

#[test]
fn member_list_uses_the_repos_workspace_unless_overridden() {
    let mut env = Env::new();
    let mock = env
        .mock("GET", "/workspaces/acme/members")
        .match_query(query(&[("pagelen", "25")]))
        .with_body(page(vec![json!({"user": account("Jane Doe", "{j}")})]).to_string())
        .create();
    let out = env.bibu().args(["member", "list"]).output().unwrap();
    mock.assert();
    assert_eq!(stdout_json(&out)[0]["display_name"], "Jane Doe");

    let mut env = Env::new();
    let other = env
        .mock("GET", "/workspaces/other/members")
        .match_query(Matcher::Any)
        .with_body(page(vec![]).to_string())
        .create();
    let out = env
        .bibu()
        .args(["member", "list", "--workspace", "other"])
        .output()
        .unwrap();
    other.assert();
    assert_eq!(stdout_json(&out), json!([]));
}

#[test]
fn member_find_matches_names_case_insensitively() {
    let mut env = Env::new();
    let mock = members_mock(&mut env);
    let out = env.bibu().args(["member", "find", "BOB"]).output().unwrap();
    mock.assert();
    let v = stdout_json(&out);
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["uuid"], "{b}");

    let mut env = Env::new();
    members_mock(&mut env);
    let out = env
        .bibu()
        .args(["member", "find", "nobody-here"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(stdout_json(&out), json!([]));
}

#[test]
fn member_commands_need_the_workspace_read_scope() {
    let mut env = Env::new();
    env.mock("GET", "/workspaces/acme/members")
        .match_query(Matcher::Any)
        .with_status(403)
        .with_body(r#"{"type":"error","error":{"message":"Your credentials lack one or more required privilege scopes.","detail":{"required":["read:workspace:bitbucket"]}}}"#)
        .create();
    let out = env.bibu().args(["member", "list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert!(stderr_json(&out)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read:workspace:bitbucket"));
}
