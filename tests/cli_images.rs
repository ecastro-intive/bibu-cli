//! `--image` uploads a file to the repository downloads and embeds it in the body.

mod common;

use common::*;
use mockito::Matcher;
use serde_json::{json, Value};

const DOWNLOADS: &str = "/repositories/acme/api/downloads";
const PR5: &str = "/repositories/acme/api/pullrequests/5";
const LINK: &str = r"!\[shot\]\(https://bitbucket\.org/acme/api/downloads/bibu-\d+-1-shot\.png\)";

fn pr_json(description: &str) -> Value {
    json!({
        "id": 5, "title": "t", "state": "OPEN", "draft": false,
        "description": description,
        "author": account("Jane Doe", "{j}"),
        "source": {"branch": {"name": "a"}},
        "destination": {"branch": {"name": "main"}},
        "comment_count": 0, "task_count": 0,
        "created_on": "2026-09-01T10:00:00+00:00", "updated_on": "2026-09-02T11:00:00+00:00",
        "links": {"html": {"href": "https://bitbucket.org/acme/api/pull-requests/5"}},
    })
}

fn comment_json(text: &str) -> Value {
    json!({
        "id": 99, "content": {"raw": text, "markup": "markdown"},
        "user": account("Jane Doe", "{j}"),
        "created_on": "2026-09-30T10:00:00+00:00", "updated_on": "2026-09-30T10:00:00+00:00",
        "deleted": false, "pending": false,
        "links": {"html": {"href": "https://bitbucket.org/acme/api/pull-requests/5/_/diff#comment-99"}},
    })
}

fn upload_mock(env: &mut Env) -> mockito::Mock {
    env.mock("POST", DOWNLOADS)
        .match_header("content-type", Matcher::Regex("multipart/form-data".into()))
        .match_body(Matcher::Regex(
            r#"name="files"; filename="bibu-\d+-1-shot\.png""#.into(),
        ))
        .with_status(201)
        .create()
}

fn image(env: &Env) -> String {
    let path = env.dir.path().join("shot.png");
    std::fs::write(&path, b"fake png bytes").unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn comment_add_uploads_then_posts_the_link() {
    let mut env = Env::new();
    let path = image(&env);
    let upload = upload_mock(&mut env);
    let post = env
        .mock("POST", &format!("{PR5}/comments"))
        .match_body(Matcher::Regex(format!(r"Before and after\\n\\n{LINK}")))
        .with_status(201)
        .with_body(comment_json("ok").to_string())
        .create();
    let out = env
        .bibu()
        .args([
            "pr",
            "comment",
            "add",
            "5",
            "Before and after",
            "--image",
            &path,
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    upload.assert();
    post.assert();
}

#[test]
fn pr_edit_appends_to_the_current_description() {
    let mut env = Env::new();
    let path = image(&env);
    let get = env.json("GET", PR5, pr_json("Old text."));
    let upload = upload_mock(&mut env);
    let put = env
        .mock("PUT", PR5)
        .match_body(Matcher::Regex(format!(
            r#""description":"Old text\.\\n\\n{LINK}""#
        )))
        .with_body(pr_json("new").to_string())
        .create();
    let out = env
        .bibu()
        .args(["pr", "edit", "5", "--image", &path])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    get.assert();
    upload.assert();
    put.assert();
}

#[test]
fn bad_images_exit_2_without_any_request() {
    let env = Env::new();
    let txt = env.dir.path().join("notes.txt");
    std::fs::write(&txt, b"x").unwrap();
    for path in [
        txt.to_string_lossy().into_owned(),
        "missing.png".to_string(),
    ] {
        let out = env
            .bibu()
            .args(["pr", "comment", "add", "5", "hi", "--image", &path])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{path}");
    }
}

#[test]
fn upload_without_write_scope_exits_4() {
    let mut env = Env::new();
    let path = image(&env);
    let upload = env
        .mock("POST", DOWNLOADS)
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(json!({"type": "error", "error": {"message": "no"}}).to_string())
        .create();
    let out = env
        .bibu()
        .args(["pr", "comment", "add", "5", "hi", "--image", &path])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    upload.assert();
}
