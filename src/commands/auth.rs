//! `bibu auth login | status | logout`

use serde::Serialize;

use crate::api::models::Account;
use crate::api::{user, Client};
use crate::auth::{BasicToken, Credentials, Source};
use crate::context::Context;
use crate::error::{BibuError, Result};
use crate::output::Render;
use crate::terminal::Terminal;

#[derive(Debug, Serialize)]
pub struct LoginResult {
    pub email: String,
    pub account: Account,
    pub stored_in: Source,
}

impl Render for LoginResult {
    fn render_table(&self) -> String {
        format!(
            "Logged in as {} ({}). Credentials saved to the {}.",
            self.account.display_name, self.email, self.stored_in
        )
    }
}

#[derive(Debug, Serialize)]
pub struct StatusResult {
    pub authenticated: bool,
    pub source: Source,
    pub email: String,
    pub account: Account,
}

impl Render for StatusResult {
    fn render_table(&self) -> String {
        format!(
            "Logged in as {} ({}) via {}.",
            self.account.display_name, self.email, self.source
        )
    }
}

#[derive(Debug, Serialize)]
pub struct LogoutResult {
    pub removed: bool,
    /// `BIBU_EMAIL` / `BIBU_TOKEN` are still set, so requests stay authenticated.
    pub env_credentials_still_set: bool,
}

impl Render for LogoutResult {
    fn render_table(&self) -> String {
        let mut text = if self.removed {
            "Logged out: stored credentials removed.".to_string()
        } else {
            "No stored credentials to remove.".to_string()
        };
        if self.env_credentials_still_set {
            text.push_str("\nNote: BIBU_EMAIL / BIBU_TOKEN are still set in this environment.");
        }
        text
    }
}

fn validate_email(email: &str) -> Result<String> {
    let email = email.trim();
    let looks_like_email = email.contains('@')
        && !email.contains(char::is_whitespace)
        && !email.starts_with('@')
        && !email.ends_with('@');
    if looks_like_email {
        Ok(email.to_string())
    } else {
        Err(BibuError::Usage(format!(
            "{email:?} is not an email address; use your Atlassian account email \
             (e.g. you@company.com), not a Bitbucket username"
        )))
    }
}

fn require_token(token: &str) -> Result<String> {
    let token = token.trim();
    if token.is_empty() {
        Err(BibuError::Usage("the API token is empty".to_string()))
    } else {
        Ok(token.to_string())
    }
}

/// Collects an email and API token, checks them against `GET /user`, then stores them.
/// Nothing is stored unless Bitbucket accepts the pair.
pub fn login(
    ctx: &Context,
    term: &dyn Terminal,
    email_flag: Option<&str>,
    with_token: bool,
) -> Result<LoginResult> {
    let token = if with_token {
        require_token(&term.read_stdin()?)?
    } else if term.is_interactive() {
        require_token(&term.prompt_secret("Bitbucket API token: ")?)?
    } else {
        return Err(BibuError::Usage(
            "no terminal available to prompt for the token; pipe it in and pass --with-token"
                .to_string(),
        ));
    };

    let email = match email_flag {
        Some(email) => validate_email(email)?,
        None if term.is_interactive() => {
            validate_email(&term.prompt_line("Atlassian account email: ")?)?
        }
        None => {
            return Err(BibuError::Usage(
                "no terminal available to prompt for the email; pass --email".to_string(),
            ))
        }
    };

    let credentials = Credentials {
        email: email.clone(),
        token,
    };
    let client = Client::new(
        &ctx.api_base,
        Box::new(BasicToken::new(credentials.clone())),
    )?;
    let account = user::current(&client)?;

    ctx.store.save(&credentials)?;
    Ok(LoginResult {
        email,
        account,
        stored_in: ctx.store.source(),
    })
}

/// Confirms the active credentials work. Not logged in or rejected → auth error (exit 3).
pub fn status(ctx: &Context) -> Result<StatusResult> {
    let (credentials, source) = ctx.credentials()?;
    let account = user::current(&ctx.client()?)?;
    Ok(StatusResult {
        authenticated: true,
        source,
        email: credentials.email,
        account,
    })
}

pub fn logout(ctx: &Context) -> Result<LogoutResult> {
    let removed = ctx.store.delete()?;
    let env_credentials_still_set = ctx.env.email.is_some() || ctx.env.token.is_some();
    Ok(LogoutResult {
        removed,
        env_credentials_still_set,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::auth::store::MemoryStore;
    use crate::auth::EnvCredentials;
    use crate::testutil::{auth_header, FakeTerminal, PIPED, TTY};

    fn ctx(server: &mockito::Server, stored: Option<Credentials>) -> Context {
        Context {
            api_base: server.url(),
            env: EnvCredentials::default(),
            store: Box::new(MemoryStore::new(stored)),
        }
    }

    fn user_body() -> String {
        json!({"display_name": "Jane Doe", "uuid": "{u-1}", "nickname": "jane"}).to_string()
    }

    #[test]
    fn login_with_piped_token_verifies_then_stores() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/user")
            .match_header(
                "authorization",
                auth_header("me@x.io", "piped-token").as_str(),
            )
            .with_body(user_body())
            .create();
        let ctx = ctx(&server, None);

        let result = login(&ctx, &PIPED, Some("me@x.io"), true).unwrap();

        mock.assert();
        assert_eq!(result.account.display_name, "Jane Doe");
        assert_eq!(
            ctx.store.load().unwrap(),
            Some(Credentials {
                email: "me@x.io".into(),
                token: "piped-token".into()
            })
        );
    }

    #[test]
    fn login_prompts_for_both_values_on_a_terminal() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/user")
            .match_header(
                "authorization",
                auth_header("typed@x.io", "typed-token").as_str(),
            )
            .with_body(user_body())
            .create();
        let ctx = ctx(&server, None);

        let result = login(&ctx, &TTY, None, false).unwrap();

        mock.assert();
        assert_eq!(result.email, "typed@x.io");
    }

    #[test]
    fn rejected_credentials_are_not_stored() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body("{}")
            .create();
        let ctx = ctx(&server, None);

        let err = login(&ctx, &PIPED, Some("me@x.io"), true).unwrap_err();

        assert!(matches!(err, BibuError::Auth(_)));
        assert_eq!(ctx.store.load().unwrap(), None);
    }

    #[test]
    fn rejected_login_keeps_the_previous_login() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body("{}")
            .create();
        let old = Credentials {
            email: "old@x.io".into(),
            token: "old".into(),
        };
        let ctx = ctx(&server, Some(old.clone()));

        assert!(login(&ctx, &PIPED, Some("me@x.io"), true).is_err());

        assert_eq!(ctx.store.load().unwrap(), Some(old));
    }

    #[test]
    fn without_a_terminal_the_token_must_be_piped() {
        let server = mockito::Server::new();
        let err = login(&ctx(&server, None), &PIPED, Some("me@x.io"), false).unwrap_err();
        assert!(matches!(err, BibuError::Usage(m) if m.contains("--with-token")));
    }

    #[test]
    fn without_a_terminal_the_email_must_be_a_flag() {
        let server = mockito::Server::new();
        let err = login(&ctx(&server, None), &PIPED, None, true).unwrap_err();
        assert!(matches!(err, BibuError::Usage(m) if m.contains("--email")));
    }

    #[test]
    fn empty_token_is_rejected_before_any_request() {
        let server = mockito::Server::new();
        let blank = FakeTerminal {
            stdin: "  \n",
            ..PIPED
        };
        let err = login(&ctx(&server, None), &blank, Some("me@x.io"), true).unwrap_err();
        assert!(matches!(err, BibuError::Usage(m) if m.contains("empty")));
    }

    #[test]
    fn email_must_look_like_an_email() {
        for bad in ["jdoe", "a b@c.io", "@x.io", "me@", ""] {
            assert!(validate_email(bad).is_err(), "{bad:?}");
        }
        assert_eq!(validate_email("  me@x.io ").unwrap(), "me@x.io");
    }

    #[test]
    fn username_hint_mentions_atlassian_email() {
        let err = validate_email("jdoe").unwrap_err();
        assert!(err.to_string().contains("Atlassian account email"));
    }

    #[test]
    fn status_reports_account_and_source() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/user").with_body(user_body()).create();
        let stored = Credentials {
            email: "me@x.io".into(),
            token: "t".into(),
        };

        let result = status(&ctx(&server, Some(stored))).unwrap();

        assert!(result.authenticated);
        assert_eq!(result.source, Source::File);
        assert_eq!(result.email, "me@x.io");
        assert_eq!(result.account.nickname.as_deref(), Some("jane"));
    }

    #[test]
    fn status_when_logged_out_is_an_auth_error() {
        let server = mockito::Server::new();
        let err = status(&ctx(&server, None)).unwrap_err();
        assert!(matches!(err, BibuError::Auth(_)));
    }

    #[test]
    fn status_with_a_revoked_token_is_an_auth_error() {
        let mut server = mockito::Server::new();
        server
            .mock("GET", "/user")
            .with_status(401)
            .with_body("{}")
            .create();
        let stored = Credentials {
            email: "me@x.io".into(),
            token: "dead".into(),
        };

        let err = status(&ctx(&server, Some(stored))).unwrap_err();

        assert!(matches!(err, BibuError::Auth(_)));
    }

    #[test]
    fn status_prefers_env_credentials() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/user")
            .match_header(
                "authorization",
                auth_header("env@x.io", "env-token").as_str(),
            )
            .with_body(user_body())
            .create();
        let mut ctx = ctx(
            &server,
            Some(Credentials {
                email: "me@x.io".into(),
                token: "t".into(),
            }),
        );
        ctx.env = EnvCredentials {
            email: Some("env@x.io".into()),
            token: Some("env-token".into()),
        };

        let result = status(&ctx).unwrap();

        mock.assert();
        assert_eq!(result.source, Source::Env);
    }

    #[test]
    fn logout_removes_and_is_idempotent() {
        let server = mockito::Server::new();
        let ctx = ctx(
            &server,
            Some(Credentials {
                email: "a@b.io".into(),
                token: "t".into(),
            }),
        );

        assert!(logout(&ctx).unwrap().removed);
        assert!(!logout(&ctx).unwrap().removed);
    }

    #[test]
    fn logout_warns_when_env_credentials_remain() {
        let server = mockito::Server::new();
        let mut ctx = ctx(&server, None);
        ctx.env = EnvCredentials {
            email: Some("e@x.io".into()),
            token: None,
        };

        let result = logout(&ctx).unwrap();

        assert!(result.env_credentials_still_set);
        assert!(result.render_table().contains("BIBU_EMAIL"));
    }
}
