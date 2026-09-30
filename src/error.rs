//! Error type shared by the whole CLI.
//!
//! Every failure maps to a stable machine-readable `code` and a process exit code, so
//! scripts and AI agents can react without parsing messages.

use serde_json::json;

/// Process exit codes. Documented in `docs/json-output.md`; treat as a public contract.
pub mod exit {
    pub const OK: i32 = 0;
    pub const OTHER: i32 = 1;
    pub const USAGE: i32 = 2;
    pub const AUTH: i32 = 3;
    pub const FORBIDDEN: i32 = 4;
    pub const NOT_FOUND: i32 = 5;
    pub const CONFLICT: i32 = 6;
    pub const NETWORK: i32 = 7;
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BibuError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Other(String),
}

impl BibuError {
    /// Stable identifier for the error class (snake_case).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            Self::Auth(_) => "auth_invalid",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::Network(_) => "network",
            Self::Other(_) => "other",
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => exit::USAGE,
            Self::Auth(_) => exit::AUTH,
            Self::Forbidden(_) => exit::FORBIDDEN,
            Self::NotFound(_) => exit::NOT_FOUND,
            Self::Conflict(_) => exit::CONFLICT,
            Self::Network(_) => exit::NETWORK,
            Self::Other(_) => exit::OTHER,
        }
    }

    /// Short actionable suggestion, when one exists.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Auth(_) => Some("run `bibu auth login` to store a valid email and API token"),
            Self::Forbidden(_) => Some("the API token is missing a required scope"),
            Self::Usage(_) => Some("run with --help to see valid usage"),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "error": {
                "code": self.code(),
                "message": self.to_string(),
                "hint": self.hint(),
            }
        })
    }
}

pub type Result<T> = std::result::Result<T, BibuError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_distinct_code_and_exit_code() {
        let all = [
            BibuError::Usage("x".into()),
            BibuError::Auth("x".into()),
            BibuError::Forbidden("x".into()),
            BibuError::NotFound("x".into()),
            BibuError::Conflict("x".into()),
            BibuError::Network("x".into()),
            BibuError::Other("x".into()),
        ];
        let mut codes: Vec<_> = all.iter().map(|e| e.code()).collect();
        let mut exits: Vec<_> = all.iter().map(|e| e.exit_code()).collect();
        codes.sort_unstable();
        codes.dedup();
        exits.sort_unstable();
        exits.dedup();
        assert_eq!(codes.len(), all.len());
        assert_eq!(exits.len(), all.len());
    }

    #[test]
    fn exit_codes_match_documented_contract() {
        assert_eq!(BibuError::Usage(String::new()).exit_code(), 2);
        assert_eq!(BibuError::Auth(String::new()).exit_code(), 3);
        assert_eq!(BibuError::Forbidden(String::new()).exit_code(), 4);
        assert_eq!(BibuError::NotFound(String::new()).exit_code(), 5);
        assert_eq!(BibuError::Conflict(String::new()).exit_code(), 6);
        assert_eq!(BibuError::Network(String::new()).exit_code(), 7);
        assert_eq!(BibuError::Other(String::new()).exit_code(), 1);
    }

    #[test]
    fn json_shape_is_stable() {
        let v = BibuError::Auth("bad token".into()).to_json();
        assert_eq!(v["error"]["code"], "auth_invalid");
        assert_eq!(v["error"]["message"], "bad token");
        assert!(v["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("bibu auth login"));
    }

    #[test]
    fn hint_is_null_when_absent() {
        let v = BibuError::NotFound("nope".into()).to_json();
        assert!(v["error"]["hint"].is_null());
    }
}
