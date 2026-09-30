//! Command-line definition (clap). Handlers live in `commands/`.

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::api::Limit;

#[derive(Debug, Parser)]
#[command(
    name = "bibu",
    version,
    about = "Bitbucket Cloud CLI for humans and AI agents",
    long_about = "Bitbucket Cloud CLI for humans and AI agents.\n\n\
        Output is a table in a terminal and JSON when piped or when --json is given. \
        Errors are JSON on stderr in that case. Exit codes are documented in \
        docs/json-output.md."
)]
pub struct Cli {
    /// Repository as `workspace/repo` or a bitbucket.org URL [default: git origin remote]
    #[arg(long, global = true)]
    pub repo: Option<String>,

    /// Force JSON output (automatic when stdout is not a terminal)
    #[arg(long, global = true)]
    pub json: bool,

    /// Skip confirmation for destructive commands (required without a terminal)
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Log in, check, or remove the stored Bitbucket credentials
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Pull requests: list, view, create, edit, review, merge
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Show which repository bibu resolved (from --repo, BIBU_REPO or the git remote)
    Repo,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Verify an Atlassian email + API token against Bitbucket, then store them
    #[command(
        long_about = "Verify an Atlassian email + API token against Bitbucket, then store them.\n\n\
        Create the token in Bitbucket: Personal settings > API tokens. Use your Atlassian \
        account email (not a Bitbucket username). The token is stored in the OS keychain.\n\n\
        Non-interactive: echo \"$TOKEN\" | bibu auth login --email you@company.com --with-token"
    )]
    Login {
        /// Atlassian account email (prompted for when omitted on a terminal)
        #[arg(long)]
        email: Option<String>,
        /// Read the API token from stdin instead of prompting
        #[arg(long)]
        with_token: bool,
    },
    /// Check that the active credentials work and show whose they are (exit 3 if not)
    Status,
    /// Remove the stored credentials (BIBU_EMAIL / BIBU_TOKEN are not affected)
    Logout,
}

/// Result-count options shared by list-style commands.
#[derive(Debug, Args)]
pub struct Paging {
    /// Maximum number of items to return
    #[arg(long, default_value_t = 25, value_parser = clap::value_parser!(u64).range(1..), conflicts_with = "all")]
    pub limit: u64,
    /// Return every item, following all pages
    #[arg(long)]
    pub all: bool,
}

impl Paging {
    pub fn limit(&self) -> Limit {
        if self.all {
            Limit::All
        } else {
            Limit::Count(self.limit as usize)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PrState {
    Open,
    Merged,
    Declined,
    Superseded,
}

impl PrState {
    /// The spelling the Bitbucket API expects.
    pub fn api_name(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Merged => "MERGED",
            Self::Declined => "DECLINED",
            Self::Superseded => "SUPERSEDED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum MergeStrategy {
    MergeCommit,
    Squash,
    FastForward,
    SquashFastForward,
    RebaseFastForward,
    RebaseMerge,
}

impl MergeStrategy {
    pub fn api_name(self) -> &'static str {
        match self {
            Self::MergeCommit => "merge_commit",
            Self::Squash => "squash",
            Self::FastForward => "fast_forward",
            Self::SquashFastForward => "squash_fast_forward",
            Self::RebaseFastForward => "rebase_fast_forward",
            Self::RebaseMerge => "rebase_merge",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum PrCommand {
    /// List pull requests (open by default), newest first
    List {
        /// Only pull requests in this state
        #[arg(long, value_enum, default_value_t = PrState::Open)]
        state: PrState,
        /// Only pull requests by this author: nickname, display name, uuid, or `me`
        #[arg(long)]
        author: Option<String>,
        /// Only pull requests from this source branch
        #[arg(long)]
        source: Option<String>,
        /// Only pull requests into this destination branch
        #[arg(long)]
        destination: Option<String>,
        /// Include reviewers and their approval state (one extra request per pull request)
        #[arg(long)]
        reviews: bool,
        #[command(flatten)]
        paging: Paging,
    },
    /// Show one pull request with its description and reviewers
    View {
        /// Pull request number
        id: u64,
    },
    /// Create a pull request (one per destination branch)
    #[command(
        long_about = "Create a pull request from the source branch into each destination.\n\n\
        The source defaults to the current git branch. Several destinations (comma-separated) \
        create one pull request each. The repository's default reviewers are added, minus you, \
        unless --no-default-reviewers is given."
    )]
    Create {
        /// Destination branch(es), comma-separated
        #[arg(long, short = 'd', required = true, value_delimiter = ',')]
        destination: Vec<String>,
        /// Source branch [default: current git branch]
        #[arg(long, short = 's')]
        source: Option<String>,
        /// Title [default: "Merge <source> into <destination>"]
        #[arg(long, short = 't')]
        title: Option<String>,
        /// Description (markdown)
        #[arg(long, short = 'm')]
        description: Option<String>,
        /// Create as a draft
        #[arg(long)]
        draft: bool,
        /// Delete the source branch when the pull request is merged
        #[arg(long)]
        close_source_branch: bool,
        /// Do not add the repository's default reviewers
        #[arg(long)]
        no_default_reviewers: bool,
    },
    /// Change a pull request's title, description, destination, draft state or branch cleanup
    Edit {
        id: u64,
        #[arg(long, short = 't')]
        title: Option<String>,
        /// New description; an empty string clears it
        #[arg(long, short = 'm')]
        description: Option<String>,
        /// New destination branch
        #[arg(long, short = 'd')]
        destination: Option<String>,
        /// Convert to a draft
        #[arg(long, conflicts_with = "ready")]
        draft: bool,
        /// Mark a draft as ready for review
        #[arg(long)]
        ready: bool,
        /// Delete the source branch when merged
        #[arg(long, conflicts_with = "keep_source_branch")]
        close_source_branch: bool,
        /// Keep the source branch when merged
        #[arg(long)]
        keep_source_branch: bool,
    },
    /// Print the pull request's unified diff (raw text unless --json)
    Diff { id: u64 },
    /// List the pull request's commits
    Commits {
        id: u64,
        #[command(flatten)]
        paging: Paging,
    },
    /// List the files changed by the pull request
    Files {
        id: u64,
        #[command(flatten)]
        paging: Paging,
    },
    /// Approve a pull request
    Approve { id: u64 },
    /// Withdraw your approval
    #[command(alias = "no-approve")]
    Unapprove { id: u64 },
    /// Request changes on a pull request
    RequestChanges { id: u64 },
    /// Withdraw your change request
    #[command(alias = "no-request-changes")]
    UnrequestChanges { id: u64 },
    /// Merge a pull request (asks for confirmation; --yes skips it)
    Merge {
        id: u64,
        /// Merge strategy [default: the repository's default]
        #[arg(long, value_enum)]
        strategy: Option<MergeStrategy>,
        /// Commit message for the merge
        #[arg(long, short = 'm')]
        message: Option<String>,
        /// Delete the source branch after merging
        #[arg(long, conflicts_with = "keep_source_branch")]
        close_source_branch: bool,
        /// Keep the source branch after merging
        #[arg(long)]
        keep_source_branch: bool,
    },
    /// Decline a pull request (asks for confirmation; --yes skips it)
    Decline { id: u64 },
}
