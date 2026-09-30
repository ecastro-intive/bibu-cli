//! Everything a command needs from the outside world, gathered once.

use crate::api::{Client, DEFAULT_API_BASE};
use crate::auth::store::default_store;
use crate::auth::{self, BasicToken, CredentialStore, Credentials, EnvCredentials, Source};
use crate::error::Result;

pub struct Context {
    pub api_base: String,
    pub env: EnvCredentials,
    pub store: Box<dyn CredentialStore>,
}

impl Context {
    pub fn from_env() -> Result<Self> {
        let api_base = std::env::var("BIBU_API_BASE")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string());
        Ok(Self {
            api_base,
            env: EnvCredentials::from_env(),
            store: default_store()?,
        })
    }

    /// The active credentials and where they came from.
    pub fn credentials(&self) -> Result<(Credentials, Source)> {
        auth::resolve(&self.env, self.store.as_ref())
    }

    /// An authenticated API client for commands that talk to Bitbucket.
    pub fn client(&self) -> Result<Client> {
        let (credentials, _) = self.credentials()?;
        Client::new(&self.api_base, Box::new(BasicToken::new(credentials)))
    }
}
