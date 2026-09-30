//! `bibu pipeline list | view | logs`

use comfy_table::presets::NOTHING;
use comfy_table::Table;
use schemars::JsonSchema;
use serde::Serialize;

use crate::api::models::pipeline::{Pipeline, Step};
use crate::api::models::Account;
use crate::api::{pipelines as api, Client};
use crate::cli::PipelineCommand;
use crate::error::{BibuError, Result};
use crate::output::{render, Mode, Render};
use crate::repo::RepoRef;

// ---------------------------------------------------------------- output types

#[derive(Debug, Serialize, JsonSchema)]
pub struct PipelineRow {
    /// Pipeline uuid, with braces.
    pub uuid: String,
    /// Run number, as shown in Bitbucket.
    pub build_number: u64,
    /// `successful`, `failed`, `stopped`, `error`, `running`, `paused`, `pending`, ...
    pub status: String,
    /// Branch (or tag) the run is for.
    pub branch: String,
    /// `branch`, `tag`, ...
    pub ref_type: String,
    /// Full hash of the commit that was built.
    pub commit: Option<String>,
    /// `PUSH`, `MANUAL`, `SCHEDULED`, ...
    pub trigger: Option<String>,
    /// Who triggered the run.
    pub creator: Option<Account>,
    /// ISO 8601 time the run was created.
    pub created_on: String,
    /// ISO 8601 time the run finished; null while it is running.
    pub completed_on: Option<String>,
    /// Build time in seconds; null until known.
    pub duration_seconds: Option<u64>,
    /// Link to the run in Bitbucket.
    pub url: String,
}

impl PipelineRow {
    fn new(p: &Pipeline, repo: &RepoRef) -> Self {
        Self {
            uuid: p.uuid.clone(),
            build_number: p.build_number,
            status: p.state.status(),
            branch: p.target.ref_name.clone(),
            ref_type: p.target.ref_type.clone(),
            commit: p
                .target
                .commit
                .as_ref()
                .map(|c| c.hash.clone())
                .filter(|h| !h.is_empty()),
            trigger: p
                .trigger
                .as_ref()
                .map(|t| t.name.clone())
                .filter(|n| !n.is_empty()),
            creator: p.creator.clone(),
            created_on: p.created_on.clone(),
            completed_on: p.completed_on.clone(),
            duration_seconds: p.duration_in_seconds,
            url: format!(
                "https://bitbucket.org/{}/{}/pipelines/results/{}",
                repo.workspace, repo.slug, p.build_number
            ),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StepRow {
    /// Step uuid, with braces.
    pub uuid: String,
    /// Step name from bitbucket-pipelines.yml.
    pub name: String,
    /// Same vocabulary as the run status.
    pub status: String,
    /// ISO 8601 time the step started; null if it has not.
    pub started_on: Option<String>,
    /// ISO 8601 time the step finished; null while running.
    pub completed_on: Option<String>,
    /// Step time in seconds; null until known.
    pub duration_seconds: Option<u64>,
}

impl From<&Step> for StepRow {
    fn from(s: &Step) -> Self {
        Self {
            uuid: s.uuid.clone(),
            name: s.name.clone(),
            status: s.state.status(),
            started_on: s.started_on.clone(),
            completed_on: s.completed_on.clone(),
            duration_seconds: s.duration_in_seconds,
        }
    }
}

fn day(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

/// `83` becomes `1m 23s`.
pub(crate) fn duration(seconds: u64) -> String {
    match (seconds / 3600, seconds % 3600 / 60, seconds % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m {s:02}s"),
        (h, m, s) => format!("{h}h {m:02}m {s:02}s"),
    }
}

fn opt_duration(seconds: Option<u64>) -> String {
    seconds.map(duration).unwrap_or_default()
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct PipelineList(pub Vec<PipelineRow>);

impl Render for PipelineList {
    fn render_table(&self) -> String {
        if self.0.is_empty() {
            return "No pipelines found.".to_string();
        }
        let mut table = Table::new();
        table.load_preset(NOTHING);
        table.set_header(vec![
            "#", "STATUS", "BRANCH", "COMMIT", "TRIGGER", "CREATOR", "CREATED", "TOOK",
        ]);
        for p in &self.0 {
            table.add_row(vec![
                format!("#{}", p.build_number),
                p.status.clone(),
                p.branch.clone(),
                p.commit
                    .as_deref()
                    .and_then(|h| h.get(..7))
                    .unwrap_or("")
                    .to_string(),
                p.trigger.clone().unwrap_or_default().to_lowercase(),
                p.creator
                    .as_ref()
                    .map(|c| c.display_name.clone())
                    .unwrap_or_default(),
                day(&p.created_on).to_string(),
                opt_duration(p.duration_seconds),
            ]);
        }
        table.to_string()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PipelineDetail {
    /// The run (its fields are flattened into this object).
    #[serde(flatten)]
    pub pipeline: PipelineRow,
    /// Steps in execution order.
    pub steps: Vec<StepRow>,
}

impl Render for PipelineDetail {
    fn render_table(&self) -> String {
        let p = &self.pipeline;
        let mut lines = vec![
            format!("#{} {} on {}", p.build_number, p.status, p.branch),
            format!(
                "Commit: {} | trigger: {} | by {}",
                p.commit.as_deref().and_then(|h| h.get(..7)).unwrap_or("?"),
                p.trigger.clone().unwrap_or_default().to_lowercase(),
                p.creator
                    .as_ref()
                    .map_or("unknown", |c| c.display_name.as_str())
            ),
            format!(
                "Created {}{}",
                day(&p.created_on),
                p.duration_seconds
                    .map(|s| format!(" | took {}", duration(s)))
                    .unwrap_or_default()
            ),
            p.url.clone(),
        ];
        if !self.steps.is_empty() {
            lines.push(String::new());
            let mut table = Table::new();
            table.load_preset(NOTHING);
            table.set_header(vec!["STEP", "NAME", "STATUS", "TOOK"]);
            for (i, s) in self.steps.iter().enumerate() {
                table.add_row(vec![
                    (i + 1).to_string(),
                    s.name.clone(),
                    s.status.clone(),
                    opt_duration(s.duration_seconds),
                ]);
            }
            lines.push(table.to_string());
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StepLog {
    /// Step uuid, with braces.
    pub uuid: String,
    /// Step name.
    pub name: String,
    /// Step status, same vocabulary as the run status.
    pub status: String,
    /// `None` when Bitbucket has no log for the step yet (for example it has not started).
    pub log: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LogsResult {
    /// Build number of the run.
    pub pipeline: u64,
    /// Logs of the selected steps, in order.
    pub steps: Vec<StepLog>,
}

impl Render for LogsResult {
    fn render_table(&self) -> String {
        let single = self.steps.len() == 1;
        self.steps
            .iter()
            .map(|s| {
                let body = s
                    .log
                    .as_deref()
                    .unwrap_or("(no log available yet)")
                    .trim_end_matches('\n');
                if single {
                    body.to_string()
                } else {
                    format!("==> {} ({}) <==\n{body}", s.name, s.status)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// --------------------------------------------------------------------- parsing

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PipelineRef {
    Number(u64),
    Uuid(String),
}

/// A build number, `{uuid}`, or a bare uuid (braces are added).
pub(crate) fn parse_ref(input: &str) -> Result<PipelineRef> {
    let input = input.trim();
    let bad = || {
        BibuError::Usage(format!(
            "{input:?} is not a pipeline: pass a build number (e.g. 42) or a uuid"
        ))
    };
    if !input.is_empty() && input.chars().all(|c| c.is_ascii_digit()) {
        return input.parse().map(PipelineRef::Number).map_err(|_| bad());
    }
    let inner = input
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(input);
    if is_uuid(inner) {
        return Ok(PipelineRef::Uuid(format!("{{{inner}}}")));
    }
    Err(bad())
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.char_indices().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

/// Picks one step by uuid, 1-based number, or (case-insensitive) name.
pub(crate) fn pick_step<'a>(steps: &'a [Step], query: &str) -> Result<&'a Step> {
    let query = query.trim();
    let unbraced = query.trim_start_matches('{').trim_end_matches('}');
    if let Some(step) = steps.iter().find(|s| {
        s.uuid
            .trim_matches(['{', '}'])
            .eq_ignore_ascii_case(unbraced)
    }) {
        return Ok(step);
    }
    if !query.is_empty() && query.chars().all(|c| c.is_ascii_digit()) {
        let number: usize = query.parse().unwrap_or(0);
        return steps.get(number.wrapping_sub(1)).ok_or_else(|| {
            BibuError::Usage(format!(
                "step {query} is out of range: this run has {} step(s)",
                steps.len()
            ))
        });
    }
    let named: Vec<(usize, &Step)> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.name.eq_ignore_ascii_case(query))
        .collect();
    match named.as_slice() {
        [(_, step)] => Ok(step),
        [] => Err(BibuError::NotFound(format!(
            "no step {query:?}; steps are: {}",
            steps
                .iter()
                .enumerate()
                .map(|(i, s)| format!("{} {:?}", i + 1, s.name))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
        many => Err(BibuError::Usage(format!(
            "{} steps are named {query:?} (numbers {}); use the number or uuid",
            many.len(),
            many.iter()
                .map(|(i, _)| (i + 1).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

// -------------------------------------------------------------------- handlers

fn resolve(client: &Client, repo: &RepoRef, id: &str) -> Result<Pipeline> {
    match parse_ref(id)? {
        PipelineRef::Number(n) => api::get_by_build_number(client, repo, n),
        PipelineRef::Uuid(uuid) => api::get(client, repo, &uuid),
    }
}

pub fn run(
    command: &PipelineCommand,
    repo: &RepoRef,
    client: &Client,
    mode: Mode,
    json: bool,
) -> Result<String> {
    match command {
        PipelineCommand::List {
            branch,
            status,
            paging,
        } => {
            let runs = api::list(client, repo, branch.as_deref(), paging.limit(), |p| {
                status.is_none_or(|s| p.state.status() == s.as_str())
            })?;
            Ok(render(
                &PipelineList(runs.iter().map(|p| PipelineRow::new(p, repo)).collect()),
                mode,
            ))
        }
        PipelineCommand::View { id } => {
            let pipeline = resolve(client, repo, id)?;
            let steps = api::steps(client, repo, &pipeline.uuid)?;
            let detail = PipelineDetail {
                pipeline: PipelineRow::new(&pipeline, repo),
                steps: steps.iter().map(StepRow::from).collect(),
            };
            Ok(render(&detail, mode))
        }
        PipelineCommand::Logs { id, step } => {
            let pipeline = resolve(client, repo, id)?;
            let all = api::steps(client, repo, &pipeline.uuid)?;
            let selected: Vec<&Step> = match step {
                Some(query) => vec![pick_step(&all, query)?],
                None => all.iter().collect(),
            };
            let mut logs = Vec::with_capacity(selected.len());
            for s in selected {
                let log = match api::step_log(client, repo, &pipeline.uuid, &s.uuid) {
                    Ok(text) => Some(text),
                    // A step that has not started has no log yet.
                    Err(BibuError::NotFound(_)) => None,
                    Err(other) => return Err(other),
                };
                logs.push(StepLog {
                    uuid: s.uuid.clone(),
                    name: s.name.clone(),
                    status: s.state.status(),
                    log,
                });
            }
            let result = LogsResult {
                pipeline: pipeline.build_number,
                steps: logs,
            };
            // Raw text by default so `bibu pipeline logs 42 | grep error` works when piped.
            Ok(render(&result, if json { Mode::Json } else { Mode::Table }))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const UUID: &str = "8f3c2a10-1b2c-4d5e-9f00-0a1b2c3d4e5f";

    fn repo() -> RepoRef {
        RepoRef {
            workspace: "acme".into(),
            slug: "api".into(),
        }
    }

    fn pipeline(v: serde_json::Value) -> Pipeline {
        serde_json::from_value(v).unwrap()
    }

    fn step(name: &str, uuid: &str) -> Step {
        serde_json::from_value(json!({"uuid": uuid, "name": name, "state": {"name": "COMPLETED", "result": {"name": "SUCCESSFUL"}}})).unwrap()
    }

    // ---- status derivation

    #[test]
    fn status_prefers_result_then_stage_then_state() {
        let cases = [
            (
                json!({"name": "COMPLETED", "result": {"name": "SUCCESSFUL"}}),
                "successful",
            ),
            (
                json!({"name": "COMPLETED", "result": {"name": "FAILED"}}),
                "failed",
            ),
            (
                json!({"name": "COMPLETED", "result": {"name": "STOPPED"}}),
                "stopped",
            ),
            (
                json!({"name": "COMPLETED", "result": {"name": "ERROR"}}),
                "error",
            ),
            (
                json!({"name": "IN_PROGRESS", "stage": {"name": "RUNNING"}}),
                "running",
            ),
            (
                json!({"name": "IN_PROGRESS", "stage": {"name": "PAUSED"}}),
                "paused",
            ),
            (json!({"name": "PENDING"}), "pending"),
            (json!({"name": "PARSING"}), "parsing"),
            (json!({}), "unknown"),
        ];
        for (state, expected) in cases {
            let p = pipeline(json!({"uuid": "{u}", "state": state}));
            assert_eq!(p.state.status(), expected);
        }
    }

    #[test]
    fn cli_status_names_match_what_status_produces() {
        use clap::ValueEnum;
        for s in crate::cli::PipelineStatus::value_variants() {
            assert_eq!(s.to_possible_value().unwrap().get_name(), s.as_str());
        }
    }

    // ---- rows and rendering

    fn sample() -> Pipeline {
        pipeline(json!({
            "uuid": "{u}", "build_number": 42,
            "creator": {"display_name": "Jane Doe", "uuid": "{j}"},
            "target": {"ref_type": "branch", "ref_name": "main", "commit": {"hash": "abcdef1234567"}},
            "trigger": {"name": "PUSH"},
            "state": {"name": "COMPLETED", "result": {"name": "FAILED"}},
            "created_on": "2026-09-30T10:00:00+00:00", "completed_on": "2026-09-30T10:01:23+00:00",
            "duration_in_seconds": 83
        }))
    }

    #[test]
    fn row_maps_fields_and_builds_the_ui_url() {
        let row = PipelineRow::new(&sample(), &repo());
        assert_eq!(
            (row.build_number, row.status.as_str(), row.branch.as_str()),
            (42, "failed", "main")
        );
        assert_eq!(row.commit.as_deref(), Some("abcdef1234567"));
        assert_eq!(row.trigger.as_deref(), Some("PUSH"));
        assert_eq!(row.duration_seconds, Some(83));
        assert_eq!(
            row.url,
            "https://bitbucket.org/acme/api/pipelines/results/42"
        );
    }

    #[test]
    fn a_bare_pipeline_still_parses() {
        let row = PipelineRow::new(&pipeline(json!({})), &repo());
        assert_eq!((row.status.as_str(), row.build_number), ("unknown", 0));
        assert!(row.commit.is_none() && row.trigger.is_none());
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(59), "59s");
        assert_eq!(duration(83), "1m 23s");
        assert_eq!(duration(3600), "1h 00m 00s");
        assert_eq!(duration(3725), "1h 02m 05s");
    }

    #[test]
    fn list_table_shows_the_columns_people_scan_for() {
        let text = PipelineList(vec![PipelineRow::new(&sample(), &repo())]).render_table();
        for needle in [
            "#42",
            "failed",
            "main",
            "abcdef1",
            "push",
            "Jane Doe",
            "2026-09-30",
            "1m 23s",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        assert_eq!(PipelineList(vec![]).render_table(), "No pipelines found.");
    }

    #[test]
    fn detail_table_lists_numbered_steps() {
        let detail = PipelineDetail {
            pipeline: PipelineRow::new(&sample(), &repo()),
            steps: vec![
                StepRow::from(&step("build", "{s1}")),
                StepRow::from(&step("test", "{s2}")),
            ],
        };
        let text = detail.render_table();
        assert!(text.contains("#42 failed on main"), "{text}");
        assert!(
            text.contains("build") && text.contains("test") && text.contains("successful"),
            "{text}"
        );
        let v = serde_json::to_value(&detail).unwrap();
        assert_eq!(v["build_number"], 42);
        assert_eq!(v["steps"][1]["name"], "test");
    }

    #[test]
    fn logs_render_bare_for_one_step_and_with_headers_for_several() {
        let one = LogsResult {
            pipeline: 42,
            steps: vec![StepLog {
                uuid: "{s}".into(),
                name: "build".into(),
                status: "failed".into(),
                log: Some("line\n\n".into()),
            }],
        };
        assert_eq!(one.render_table(), "line");
        let two = LogsResult {
            pipeline: 42,
            steps: vec![
                StepLog {
                    uuid: "{a}".into(),
                    name: "build".into(),
                    status: "successful".into(),
                    log: Some("ok\n".into()),
                },
                StepLog {
                    uuid: "{b}".into(),
                    name: "deploy".into(),
                    status: "pending".into(),
                    log: None,
                },
            ],
        };
        assert_eq!(
            two.render_table(),
            "==> build (successful) <==\nok\n\n==> deploy (pending) <==\n(no log available yet)"
        );
    }

    // ---- parsing

    #[test]
    fn refs_accept_numbers_and_uuids() {
        assert_eq!(parse_ref("42").unwrap(), PipelineRef::Number(42));
        assert_eq!(parse_ref(" 7 ").unwrap(), PipelineRef::Number(7));
        let braced = format!("{{{UUID}}}");
        assert_eq!(
            parse_ref(&braced).unwrap(),
            PipelineRef::Uuid(braced.clone())
        );
        assert_eq!(parse_ref(UUID).unwrap(), PipelineRef::Uuid(braced));
    }

    #[test]
    fn refs_reject_everything_else() {
        for bad in [
            "",
            "abc",
            "4x2",
            "-1",
            "{123}",
            "8f3c2a10-1b2c-4d5e-9f00",
            "{8f3c2a10-1b2c-4d5e-9f00-0a1b2c3d4e5}",
            "99999999999999999999999",
        ] {
            assert!(
                matches!(parse_ref(bad), Err(BibuError::Usage(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn steps_are_picked_by_uuid_number_or_name() {
        let steps = vec![
            step("build", "{aaa}"),
            step("Test", "{bbb}"),
            step("deploy", "{ccc}"),
        ];
        assert_eq!(pick_step(&steps, "{bbb}").unwrap().name, "Test");
        assert_eq!(pick_step(&steps, "BBB").unwrap().name, "Test");
        assert_eq!(pick_step(&steps, "1").unwrap().name, "build");
        assert_eq!(pick_step(&steps, "3").unwrap().name, "deploy");
        assert_eq!(pick_step(&steps, "test").unwrap().name, "Test");
    }

    #[test]
    fn step_errors_are_specific() {
        let steps = vec![step("build", "{aaa}"), step("build", "{bbb}")];
        assert!(
            matches!(pick_step(&steps, "0"), Err(BibuError::Usage(m)) if m.contains("out of range"))
        );
        assert!(matches!(pick_step(&steps, "3"), Err(BibuError::Usage(m)) if m.contains("2 step")));
        assert!(
            matches!(pick_step(&steps, "build"), Err(BibuError::Usage(m)) if m.contains("1, 2"))
        );
        assert!(
            matches!(pick_step(&steps, "nope"), Err(BibuError::NotFound(m)) if m.contains("1 \"build\""))
        );
        assert!(matches!(pick_step(&[], "1"), Err(BibuError::Usage(_))));
    }
}
