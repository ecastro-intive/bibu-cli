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
    /// Bitbucket Pipelines: list runs, inspect one, read step logs
    Pipeline {
        #[command(subcommand)]
        command: PipelineCommand,
    },
    /// Branches: list, create, delete
    Branch {
        #[command(subcommand)]
        command: BranchCommand,
    },
    /// Workspace members (used to find people for reviewers)
    Member {
        /// Workspace slug [default: the workspace of the resolved repository]
        #[arg(long, global = true)]
        workspace: Option<String>,
        #[command(subcommand)]
        command: MemberCommand,
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
    /// Comments: general, inline on a file or line, replies, resolving
    Comment {
        #[command(subcommand)]
        command: CommentCommand,
    },
    /// Reviewers: list, add, remove
    Reviewers {
        #[command(subcommand)]
        command: ReviewersCommand,
    },
    /// Tasks attached to a pull request
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    /// List comments as threads, oldest first (deleted ones are hidden)
    List {
        /// Pull request number
        id: u64,
        /// Only threads whose first comment is not resolved
        #[arg(long)]
        unresolved: bool,
        /// Only inline comments (anchored to a file)
        #[arg(long)]
        inline: bool,
        /// Only threads anchored to this file path (implies --inline)
        #[arg(long)]
        file: Option<String>,
        /// Also show deleted comments
        #[arg(long)]
        include_deleted: bool,
        #[command(flatten)]
        paging: Paging,
    },
    /// Add a comment: general, on a file, or on a line or line range
    #[command(long_about = "Add a comment to a pull request.\n\n\
        Without --file the comment is general. With --file it is attached to that file; add --line \
        to attach it to a line of the new version, --end-line for a range, and --old-side to \
        anchor to the old version (for removed lines). The text is the last argument; pass - or \
        omit it to read the text from stdin.\n\n\
        Examples:\n  bibu pr comment add 12 \"Looks good overall\"\n  \
        bibu pr comment add 12 --file src/app.py --line 30 \"Why this default?\"\n  \
        echo \"long text\" | bibu pr comment add 12 --file src/app.py --line 30 --end-line 35")]
    Add {
        id: u64,
        /// Comment text (markdown); `-` or omitted reads stdin
        text: Option<String>,
        /// File path, relative to the repository root
        #[arg(long, short = 'f')]
        file: Option<String>,
        /// Line number (requires --file)
        #[arg(long, short = 'l', requires = "file", value_parser = clap::value_parser!(u64).range(1..))]
        line: Option<u64>,
        /// Last line of a range that starts at --line
        #[arg(long, requires = "line", value_parser = clap::value_parser!(u64).range(1..))]
        end_line: Option<u64>,
        /// Anchor to the old version of the file instead of the new one
        #[arg(long, requires = "line")]
        old_side: bool,
    },
    /// Reply to a comment
    Reply {
        id: u64,
        /// Number of the comment to reply to
        comment_id: u64,
        /// Reply text (markdown); `-` or omitted reads stdin
        text: Option<String>,
    },
    /// Change the text of one of your comments
    Edit {
        id: u64,
        comment_id: u64,
        /// New text (markdown); `-` or omitted reads stdin
        text: Option<String>,
    },
    /// Delete one of your comments (asks for confirmation; --yes skips it)
    Delete { id: u64, comment_id: u64 },
    /// Mark a comment thread as resolved
    Resolve { id: u64, comment_id: u64 },
    /// Reopen a resolved comment thread
    Reopen { id: u64, comment_id: u64 },
}

#[derive(Debug, Subcommand)]
pub enum ReviewersCommand {
    /// List reviewers and their review state
    List { id: u64 },
    /// Add reviewers, keeping the existing ones
    #[command(long_about = "Add reviewers, keeping the existing ones.\n\n\
        Each person is a name, nickname, account id, `{uuid}`, or `me`; names are looked up in the \
        workspace members and must match exactly one person. Bitbucket replaces the whole list on \
        update, so bibu reads the current reviewers first and sends the merged list.")]
    Add {
        id: u64,
        /// People to add
        #[arg(required = true)]
        users: Vec<String>,
    },
    /// Remove reviewers, keeping the others
    Remove {
        id: u64,
        /// People to remove (name, nickname, account id or `{uuid}`)
        #[arg(required = true)]
        users: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// List a pull request's tasks
    List {
        id: u64,
        /// Only tasks that are not resolved
        #[arg(long)]
        unresolved: bool,
        #[command(flatten)]
        paging: Paging,
    },
    /// Add a task, optionally attached to a comment
    Add {
        id: u64,
        /// Task text; `-` or omitted reads stdin
        text: Option<String>,
        /// Attach the task to this comment
        #[arg(long)]
        comment: Option<u64>,
    },
    /// Mark a task resolved
    Resolve { id: u64, task_id: u64 },
    /// Mark a resolved task unresolved again
    Reopen { id: u64, task_id: u64 },
}

#[derive(Debug, Subcommand)]
pub enum MemberCommand {
    /// List workspace members
    List {
        #[command(flatten)]
        paging: Paging,
    },
    /// Find members by name, nickname, account id or uuid
    Find {
        /// Text to look for (case-insensitive)
        query: String,
    },
}

/// Pipeline status as `bibu` reports it (lowercase). Finished runs report their result, running
/// ones their stage, the rest their state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PipelineStatus {
    Pending,
    Parsing,
    Running,
    Paused,
    Halted,
    Successful,
    Failed,
    Stopped,
    Error,
}

impl PipelineStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Parsing => "parsing",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Halted => "halted",
            Self::Successful => "successful",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    /// List pipeline runs, newest first
    List {
        /// Only runs of this branch
        #[arg(long)]
        branch: Option<String>,
        /// Only runs with this status
        #[arg(long, value_enum)]
        status: Option<PipelineStatus>,
        #[command(flatten)]
        paging: Paging,
    },
    /// Show one run and its steps
    #[command(long_about = "Show one pipeline run and its steps.\n\n\
        The run is a build number (as in the Bitbucket UI, e.g. 42) or a uuid.")]
    View {
        /// Build number or uuid
        id: String,
    },
    /// Print step logs (raw text unless --json)
    #[command(long_about = "Print the logs of a pipeline run.\n\n\
        Without --step every step's log is printed under a header; with --step only that step. \
        A step is its name, its 1-based position, or its uuid. Output is raw text even when piped \
        so it can go through grep; add --json for {pipeline, steps: [{name, status, log}]}.")]
    Logs {
        /// Build number or uuid
        id: String,
        /// Only this step: name, 1-based number, or uuid
        #[arg(long)]
        step: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchCommand {
    /// List branches, most recently updated first
    List {
        /// Only branches whose name contains this text (case-insensitive)
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        paging: Paging,
    },
    /// Create a branch from a branch or a commit
    Create {
        /// Name of the new branch
        name: String,
        /// Branch name or commit hash to start from [default: the repository's default branch]
        #[arg(long)]
        from: Option<String>,
    },
    /// Delete a branch (asks for confirmation; --yes skips it; never the default branch)
    Delete {
        /// Branch name
        name: String,
    },
}
