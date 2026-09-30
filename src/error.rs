//! Error type shared by the whole CLI.
//!
//! Every failure maps to a stable machine-readable `code` and a process exit code, so
//! scripts and AI agents can react without parsing messages.

use schemars::JsonSchema;
use serde::Serialize;

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

/// One row of the public exit-code contract, shared by `bibu schema` and the generated docs.
pub struct ExitInfo {
    pub code: i32,
    /// The `error.code` string that comes with it, or `none` for success.
    pub name: &'static str,
    pub meaning: &'static str,
}

pub const EXIT_CODES: &[ExitInfo] = &[
    ExitInfo { code: exit::OK, name: "none", meaning: "success" },
    ExitInfo { code: exit::OTHER, name: "other", meaning: "unexpected failure (I/O, unreadable credentials file, unexpected API response)" },
    ExitInfo { code: exit::USAGE, name: "usage", meaning: "bad arguments, unknown flag, missing confirmation (--yes), or no repository" },
    ExitInfo { code: exit::AUTH, name: "auth_invalid", meaning: "not logged in, or Bitbucket rejected the credentials (401)" },
    ExitInfo { code: exit::FORBIDDEN, name: "forbidden", meaning: "the API token lacks a scope or permission (403); the message names the scopes" },
    ExitInfo { code: exit::NOT_FOUND, name: "not_found", meaning: "the pull request, comment, branch, pipeline, person, ... does not exist (404)" },
    ExitInfo { code: exit::CONFLICT, name: "conflict", meaning: "Bitbucket refused the change: invalid input, already exists, merge conflict (400/409/422)" },
    ExitInfo { code: exit::NETWORK, name: "network", meaning: "cannot reach Bitbucket, rate limited (429), or a server error (5xx); retrying later may work" },
];

/// What `bibu` prints on stderr (as one line of JSON) when a command fails and output is JSON.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorEnvelope {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorDetail {
    /// Stable identifier of the error class; see the exit-code table.
    pub code: &'static str,
    /// Human readable explanation. Do not parse it.
    pub message: String,
    /// A suggested next step, when there is one.
    pub hint: Option<&'static str>,
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

    pub fn envelope(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorDetail {
                code: self.code(),
                message: self.to_string(),
                hint: self.hint(),
            },
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self.envelope()).expect("error envelope serializes")
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
    fn exit_code_table_matches_the_error_variants() {
        let all = [
            BibuError::Usage(String::new()),
            BibuError::Auth(String::new()),
            BibuError::Forbidden(String::new()),
            BibuError::NotFound(String::new()),
            BibuError::Conflict(String::new()),
            BibuError::Network(String::new()),
            BibuError::Other(String::new()),
        ];
        for error in &all {
            let row = EXIT_CODES
                .iter()
                .find(|r| r.code == error.exit_code())
                .expect("documented");
            assert_eq!(row.name, error.code());
        }
        assert_eq!(
            EXIT_CODES.len(),
            all.len() + 1,
            "one row per error plus success"
        );
        assert_eq!(EXIT_CODES[0].code, exit::OK);
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
