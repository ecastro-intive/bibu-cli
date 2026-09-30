//! Pull request resources. Lenient by design: a missing optional field must not make a whole
//! listing fail to parse.

use serde::Deserialize;

use super::Account;

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub state: String,
    /// Plain `description` on responses; older payloads only carry `summary.raw`.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub summary: Option<Markup>,
    #[serde(default)]
    pub author: Option<Account>,
    #[serde(default)]
    pub source: Endpoint,
    #[serde(default)]
    pub destination: Endpoint,
    #[serde(default)]
    pub merge_commit: Option<CommitRef>,
    #[serde(default)]
    pub comment_count: u64,
    #[serde(default)]
    pub task_count: u64,
    #[serde(default)]
    pub close_source_branch: bool,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub created_on: String,
    #[serde(default)]
    pub updated_on: String,
    /// Only present when fetching a single PR by id.
    #[serde(default)]
    pub reviewers: Vec<Account>,
    /// Only present when fetching a single PR by id.
    #[serde(default)]
    pub participants: Vec<Participant>,
    #[serde(default)]
    pub links: Links,
}

impl PullRequest {
    pub fn description_text(&self) -> String {
        self.description
            .clone()
            .or_else(|| self.summary.as_ref().map(|m| m.raw.clone()))
            .unwrap_or_default()
    }

    pub fn url(&self) -> String {
        self.links
            .html
            .as_ref()
            .map(|l| l.href.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Markup {
    #[serde(default)]
    pub raw: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Endpoint {
    #[serde(default)]
    pub branch: Option<BranchRef>,
    #[serde(default)]
    pub commit: Option<CommitRef>,
}

impl Endpoint {
    pub fn branch_name(&self) -> String {
        self.branch
            .as_ref()
            .map(|b| b.name.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BranchRef {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitRef {
    #[serde(default)]
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Participant {
    #[serde(default)]
    pub user: Account,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub approved: bool,
    /// `approved`, `changes_requested` or null.
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Links {
    #[serde(default)]
    pub html: Option<Href>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Href {
    #[serde(default)]
    pub href: String,
}

/// One entry of `GET .../diffstat`.
#[derive(Debug, Clone, Deserialize)]
pub struct DiffStat {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub lines_added: u64,
    #[serde(default)]
    pub lines_removed: u64,
    #[serde(default)]
    pub old: Option<FilePath>,
    #[serde(default)]
    pub new: Option<FilePath>,
}

impl DiffStat {
    /// The file's current path, or its old path when it was removed.
    pub fn path(&self) -> String {
        self.new
            .as_ref()
            .or(self.old.as_ref())
            .map(|f| f.path.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilePath {
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Commit {
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub author: CommitAuthor,
    #[serde(default)]
    pub links: Links,
}

impl Commit {
    pub fn subject(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CommitAuthor {
    /// `Name <email>` as recorded in git.
    #[serde(default)]
    pub raw: String,
    #[serde(default)]
    pub user: Option<Account>,
}

impl CommitAuthor {
    pub fn name(&self) -> String {
        match &self.user {
            Some(user) if !user.display_name.is_empty() => user.display_name.clone(),
            _ => self.raw.split('<').next().unwrap_or("").trim().to_string(),
        }
    }
}
