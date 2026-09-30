//! Typed API resources. Only fields bibu reads are modelled; unknown fields are ignored.

use serde::{Deserialize, Serialize};

pub mod pullrequest;

/// A Bitbucket user, as returned by `GET /user`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}
