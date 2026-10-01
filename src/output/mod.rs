//! Output rendering. One value, two views: a human table or JSON for machines.

pub mod table;

use std::io::IsTerminal;

use serde::Serialize;

use crate::error::BibuError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Table,
    Json,
}

impl Mode {
    /// JSON when `--json` is passed or stdout is not a terminal (piped / agent use).
    pub fn detect(force_json: bool) -> Self {
        Self::resolve(force_json, std::io::stdout().is_terminal())
    }

    pub fn resolve(force_json: bool, stdout_is_tty: bool) -> Self {
        if force_json || !stdout_is_tty {
            Self::Json
        } else {
            Self::Table
        }
    }
}

/// Anything a command returns. JSON comes from `Serialize`; the table is hand-written.
pub trait Render: Serialize {
    fn render_table(&self) -> String;
}

pub fn render<T: Render>(value: &T, mode: Mode) -> String {
    match mode {
        Mode::Table => value.render_table(),
        Mode::Json => serde_json::to_string_pretty(value).expect("output types are serializable"),
    }
}

pub fn render_error(err: &BibuError, mode: Mode) -> String {
    match mode {
        Mode::Json => err.to_json().to_string(),
        Mode::Table => match err.hint() {
            Some(hint) => format!("error: {err}\nhint: {hint}"),
            None => format!("error: {err}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Sample {
        id: u32,
    }

    impl Render for Sample {
        fn render_table(&self) -> String {
            format!("id={}", self.id)
        }
    }

    #[test]
    fn mode_is_json_when_forced_or_piped() {
        assert_eq!(Mode::resolve(true, true), Mode::Json);
        assert_eq!(Mode::resolve(false, false), Mode::Json);
        assert_eq!(Mode::resolve(false, true), Mode::Table);
    }

    #[test]
    fn render_uses_requested_view() {
        assert_eq!(render(&Sample { id: 7 }, Mode::Table), "id=7");
        let json: serde_json::Value =
            serde_json::from_str(&render(&Sample { id: 7 }, Mode::Json)).unwrap();
        assert_eq!(json["id"], 7);
    }

    #[test]
    fn error_table_includes_hint_only_when_present() {
        let with = render_error(&BibuError::Auth("bad".into()), Mode::Table);
        assert!(with.starts_with("error: bad\nhint: "));
        let without = render_error(&BibuError::NotFound("gone".into()), Mode::Table);
        assert_eq!(without, "error: gone");
    }

    #[test]
    fn error_json_is_single_line_valid_json() {
        let s = render_error(&BibuError::Conflict("c".into()), Mode::Json);
        assert!(!s.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["error"]["code"], "conflict");
    }
}
