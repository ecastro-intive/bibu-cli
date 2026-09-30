//! Authentication: Atlassian email + API token over HTTP Basic auth.
//!
//! Credential precedence: `BIBU_EMAIL` + `BIBU_TOKEN` env vars, then the stored login
//! (OS keychain, or the file named by `BIBU_CREDENTIALS_FILE`).

pub mod store;

use std::fmt;

use reqwest::blocking::RequestBuilder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{BibuError, Result};
pub use store::{CredentialStore, FileStore};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub token: String,
}

/// The token must never reach logs or error output, so `Debug` redacts it.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("email", &self.email)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Where the active credentials came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Env,
    Keychain,
    File,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Env => "environment variables",
            Self::Keychain => "OS keychain",
            Self::File => "credentials file",
        })
    }
}

/// Signs outgoing requests. A trait so Bearer or OAuth can be added without touching callers.
pub trait Authenticator: Send + Sync {
    fn apply(&self, req: RequestBuilder) -> RequestBuilder;
}

pub struct BasicToken(Credentials);

impl BasicToken {
    pub fn new(credentials: Credentials) -> Self {
        Self(credentials)
    }
}

impl Authenticator for BasicToken {
    fn apply(&self, req: RequestBuilder) -> RequestBuilder {
        req.basic_auth(&self.0.email, Some(&self.0.token))
    }
}

/// Credentials supplied through the environment, if any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvCredentials {
    pub email: Option<String>,
    pub token: Option<String>,
}

impl EnvCredentials {
    pub fn from_env() -> Self {
        let read = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Self {
            email: read("BIBU_EMAIL"),
            token: read("BIBU_TOKEN"),
        }
    }
}

/// Picks the active credentials. Env vars win; half-set env vars are an error, not a
/// silent fallback, because that would hide a misconfigured CI job behind a stale login.
pub fn resolve(env: &EnvCredentials, store: &dyn CredentialStore) -> Result<(Credentials, Source)> {
    match (&env.email, &env.token) {
        (Some(email), Some(token)) => Ok((
            Credentials {
                email: email.clone(),
                token: token.clone(),
            },
            Source::Env,
        )),
        (Some(_), None) | (None, Some(_)) => Err(BibuError::Usage(
            "BIBU_EMAIL and BIBU_TOKEN must be set together".to_string(),
        )),
        (None, None) => match store.load()? {
            Some(credentials) => Ok((credentials, store.source())),
            None => Err(BibuError::Auth("not logged in".to_string())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::store::MemoryStore;
    use super::*;

    fn creds(email: &str) -> Credentials {
        Credentials {
            email: email.into(),
            token: "tok".into(),
        }
    }

    fn env(email: Option<&str>, token: Option<&str>) -> EnvCredentials {
        EnvCredentials {
            email: email.map(Into::into),
            token: token.map(Into::into),
        }
    }

    #[test]
    fn env_beats_store() {
        let store = MemoryStore::new(Some(creds("stored@x.io")));
        let (got, source) = resolve(&env(Some("env@x.io"), Some("etok")), &store).unwrap();
        assert_eq!(got.email, "env@x.io");
        assert_eq!(got.token, "etok");
        assert_eq!(source, Source::Env);
    }

    #[test]
    fn store_used_when_env_empty() {
        let store = MemoryStore::new(Some(creds("stored@x.io")));
        let (got, source) = resolve(&env(None, None), &store).unwrap();
        assert_eq!(got.email, "stored@x.io");
        assert_eq!(source, Source::File);
    }

    #[test]
    fn half_set_env_is_a_usage_error_even_with_a_stored_login() {
        let store = MemoryStore::new(Some(creds("stored@x.io")));
        for partial in [env(Some("a@b.io"), None), env(None, Some("t"))] {
            assert!(matches!(
                resolve(&partial, &store),
                Err(BibuError::Usage(_))
            ));
        }
    }

    #[test]
    fn nothing_anywhere_is_an_auth_error() {
        let store = MemoryStore::new(None);
        assert!(matches!(
            resolve(&env(None, None), &store),
            Err(BibuError::Auth(_))
        ));
    }

    #[test]
    fn debug_output_redacts_the_token() {
        let shown = format!(
            "{:?}",
            Credentials {
                email: "a@b.io".into(),
                token: "s3cret".into()
            }
        );
        assert!(shown.contains("a@b.io"));
        assert!(!shown.contains("s3cret"));
    }
}
