//! Bitbucket Pipelines resources. Lenient like the other models.

use serde::Deserialize;

use super::pullrequest::CommitRef;
use super::Account;

/// `state` of a pipeline or step. Completed states carry a `result`, running ones a `stage`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct State {
    /// `PENDING`, `IN_PROGRESS`, `COMPLETED`, ...
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub result: Option<Named>,
    #[serde(default)]
    pub stage: Option<Named>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Named {
    #[serde(default)]
    pub name: String,
}

impl State {
    /// One lowercase word for humans and scripts: the result once finished (`successful`,
    /// `failed`, `stopped`, `error`), else the stage (`running`, `paused`), else the state
    /// (`pending`, `parsing`).
    pub fn status(&self) -> String {
        let raw = self
            .result
            .as_ref()
            .map(|r| r.name.as_str())
            .filter(|n| !n.is_empty())
            .or_else(|| {
                self.stage
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .filter(|n| !n.is_empty())
            })
            .unwrap_or(self.name.as_str());
        if raw.is_empty() {
            "unknown".to_string()
        } else {
            raw.to_lowercase()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pipeline {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub build_number: u64,
    #[serde(default)]
    pub creator: Option<Account>,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub trigger: Option<Named>,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub created_on: String,
    #[serde(default)]
    pub completed_on: Option<String>,
    #[serde(default)]
    pub duration_in_seconds: Option<u64>,
    #[serde(default)]
    pub build_seconds_used: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub ref_type: String,
    #[serde(default)]
    pub ref_name: String,
    #[serde(default)]
    pub commit: Option<CommitRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub started_on: Option<String>,
    #[serde(default)]
    pub completed_on: Option<String>,
    #[serde(default)]
    pub duration_in_seconds: Option<u64>,
}
