//! Pull request comments. Lenient like the other models.

use serde::Deserialize;

use super::pullrequest::Links;
use super::Account;

#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub created_on: String,
    #[serde(default)]
    pub updated_on: String,
    #[serde(default)]
    pub content: Content,
    #[serde(default)]
    pub user: Option<Account>,
    #[serde(default)]
    pub deleted: bool,
    /// Draft comments of an unfinished review.
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub parent: Option<ParentRef>,
    #[serde(default)]
    pub inline: Option<Inline>,
    /// Present (even as an empty object) once the thread is resolved.
    #[serde(default)]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub links: Links,
}

impl Comment {
    pub fn url(&self) -> String {
        self.links
            .html
            .as_ref()
            .map(|l| l.href.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Content {
    #[serde(default)]
    pub raw: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParentRef {
    pub id: u64,
}

/// Where a comment is anchored. `to`/`start_to` refer to the new version of the file,
/// `from`/`start_from` to the old one.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Inline {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub from: Option<u64>,
    #[serde(default)]
    pub to: Option<u64>,
    #[serde(default)]
    pub start_from: Option<u64>,
    #[serde(default)]
    pub start_to: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Resolution {
    #[serde(default)]
    pub user: Option<Account>,
    #[serde(default)]
    pub created_on: String,
}
