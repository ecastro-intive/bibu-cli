//! Branches and the bits of a repository bibu needs.

use serde::Deserialize;

use super::pullrequest::Commit;

#[derive(Debug, Clone, Deserialize)]
pub struct Branch {
    pub name: String,
    /// The branch tip.
    #[serde(default)]
    pub target: Option<Commit>,
}

/// Just enough of `GET /repositories/{workspace}/{slug}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Repository {
    #[serde(default)]
    pub mainbranch: Option<MainBranch>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MainBranch {
    #[serde(default)]
    pub name: String,
}

impl Repository {
    pub fn default_branch(&self) -> Option<&str> {
        self.mainbranch
            .as_ref()
            .map(|b| b.name.as_str())
            .filter(|n| !n.is_empty())
    }
}
