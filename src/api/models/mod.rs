//! Typed API resources. Only fields bibu reads are modelled; unknown fields are ignored.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod branch;
pub mod comment;
pub mod pipeline;
pub mod pullrequest;
pub mod task;

/// A Bitbucket user, as returned by `GET /user`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Account {
    /// Display name.
    #[serde(default)]
    pub display_name: String,
    /// Bitbucket uuid, with braces.
    #[serde(default)]
    pub uuid: String,
    /// Short handle, when the account has one.
    #[serde(default)]
    pub nickname: Option<String>,
    /// Atlassian account id.
    #[serde(default)]
    pub account_id: Option<String>,
}

/// An entry of `GET /workspaces/{workspace}/members`.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceMember {
    #[serde(default)]
    pub user: Account,
}
