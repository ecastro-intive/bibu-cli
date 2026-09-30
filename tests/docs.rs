//! The reference docs are generated from the CLI definition; this fails when they are stale.
//!
//! Regenerate with: `UPDATE_DOCS=1 cargo test --test docs`

use std::path::Path;

fn check(file: &str, expected: String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    if std::env::var_os("UPDATE_DOCS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &expected).unwrap();
        return;
    }
    // A Windows checkout may have converted line endings.
    let actual = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        actual == expected,
        "{file} is out of date with the CLI definition.\n\
         Regenerate it with: UPDATE_DOCS=1 cargo test --test docs"
    );
}

#[test]
fn command_reference_is_up_to_date() {
    check("docs/commands.md", bibu::schema::markdown::commands());
}

#[test]
fn output_and_error_reference_is_up_to_date() {
    check("docs/json-output.md", bibu::schema::markdown::json_output());
}
