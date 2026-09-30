//! Pull request tasks.

use serde::Deserialize;

use super::comment::{Content, ParentRef};
use super::Account;

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub id: u64,
    /// `RESOLVED` or `UNRESOLVED`.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub content: Content,
    #[serde(default)]
    pub creator: Option<Account>,
    #[serde(default)]
    pub created_on: String,
    #[serde(default)]
    pub updated_on: String,
    #[serde(default)]
    pub resolved_on: Option<String>,
    #[serde(default)]
    pub resolved_by: Option<Account>,
    /// The comment this task hangs off, if any.
    #[serde(default)]
    pub comment: Option<ParentRef>,
}
