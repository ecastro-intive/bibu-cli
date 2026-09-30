# bibu

A Bitbucket Cloud CLI for humans and AI agents, written in Rust. Successor to bb-cli.

**Status:** under construction (milestone 6 of 7). Available so far: `bibu auth`, `bibu pr` (core, comments, reviewers, tasks), `bibu pipeline`, `bibu branch`, `bibu member` and `bibu repo`.

## Install

Releases are published on GitHub for macOS (Apple Silicon and Intel) and Windows (x64).

```sh
# macOS
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ecastro-intive/bibu-cli/releases/latest/download/bibu-installer.sh | sh
```

```powershell
# Windows (PowerShell)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/ecastro-intive/bibu-cli/releases/latest/download/bibu-installer.ps1 | iex"
```

The installer puts `bibu` in `~/.local/bin` (make sure that is on your `PATH`), verifies the download
checksum, and records how bibu was installed so it can be upgraded later. Check with `bibu --version`.
To upgrade later, run `bibu upgrade` (or `bibu upgrade --check` to only look). It works for copies
that were installed with the installer; a copy built from source is told how to get one.

The binaries are **not code-signed**. The installers download with `curl` / PowerShell, which macOS
does not flag. If you download an archive with a browser instead, macOS may refuse to open it; clear
the flag with `xattr -d com.apple.quarantine bibu`.

From source (needs Rust 1.85 or newer): `cargo install --git https://github.com/ecastro-intive/bibu-cli`.

## For people and for agents

- **Reference:** every command, argument and output field is documented in
  [docs/commands.md](docs/commands.md); output modes, errors and exit codes are in
  [docs/json-output.md](docs/json-output.md). Both are generated from the CLI definition and checked
  in CI, so they cannot go stale.
- **Machine-readable:** `bibu schema` prints the command tree as JSON; `bibu schema pr comment add`
  adds that command's arguments and the JSON Schema of its output. `bibu schema --full` includes every
  output schema.
- **Claude skill:** [skills/bibu/SKILL.md](skills/bibu/SKILL.md) teaches Claude Code how to use bibu
  safely (always `--json`, decide on exit codes, only merge or delete when asked, typical review and
  pipeline workflows). Every command example in it is tested against the real CLI. Install it once:

```sh
mkdir -p ~/.claude/skills && cp -R skills/bibu ~/.claude/skills/        # macOS / Linux
```

```powershell
Copy-Item -Recurse skills\bibu $HOME\.claude\skills\                   # Windows (PowerShell)
```

  Agents and scripts should authenticate with `BIBU_EMAIL` and `BIBU_TOKEN` instead of the keychain.

## Output contract

- In a terminal, output is a table. When stdout is piped, or `--json` is passed, output is JSON.
- Results go to stdout. Errors go to stderr, as JSON in JSON mode:
  `{"error":{"code":"usage","message":"...","hint":"..."}}`
- Exit codes: `0` ok, `1` other, `2` usage, `3` auth, `4` forbidden / missing scope,
  `5` not found, `6` conflict or validation, `7` network.

## Authentication

bibu logs in with your **Atlassian account email** and a Bitbucket **API token**
(app passwords are retired). Create the token in Bitbucket under *Personal settings > API tokens*
and grant the scopes for the commands you use. `bibu auth status` needs `read:user:bitbucket`.

```sh
bibu auth login                      # prompts for email and (hidden) token
echo "$TOKEN" | bibu auth login --email you@company.com --with-token   # non-interactive
bibu auth status                     # verifies the login against Bitbucket; exit 3 if invalid
bibu auth logout                     # removes the stored login
```

`login` checks the pair against `GET /user` and stores nothing unless Bitbucket accepts it.
Credentials are kept in the OS keychain (macOS Keychain / Windows Credential Manager).
On macOS the Keychain ties access to the exact binary, so after a rebuild or an unsigned upgrade it
may ask once whether `bibu` can use the stored item (choose *Always Allow*). Scripts and agents should
use `BIBU_EMAIL` and `BIBU_TOKEN`, which never touch the Keychain.

| Variable | Purpose |
|---|---|
| `BIBU_EMAIL` + `BIBU_TOKEN` | Credentials for CI and agents. Win over the stored login. Must be set together. |
| `BIBU_CREDENTIALS_FILE` | Store the login in this JSON file (mode `0600`) instead of the keychain. |
| `BIBU_API_BASE` | Override the API root (`https://api.bitbucket.org/2.0`). For tests. |

A 403 names the missing scopes when Bitbucket reports them (exit 4).

## Pull requests

All `bibu pr` commands take `--repo workspace/repo` (or use `BIBU_REPO` / the git remote) and
`--json`. Pull request numbers are plain integers.

| Command | What it does |
|---|---|
| `bibu pr list [--state open\|merged\|declined\|superseded] [--author NAME\|me] [--source B] [--destination B] [--reviews] [--limit N \| --all]` | List pull requests. Filters apply across all pages. `--reviews` adds each reviewer's state (one extra request per PR). |
| `bibu pr view ID` | Details, description, and reviewers with their approval state. |
| `bibu pr create -d main[,develop] [-s BRANCH] [-t TITLE] [-m DESC] [--draft] [--close-source-branch] [--no-default-reviewers]` | One PR per destination. Source defaults to the current git branch. Adds the repo's default reviewers (minus you). |
| `bibu pr edit ID [-t] [-m] [-d BRANCH] [--draft \| --ready] [--close-source-branch \| --keep-source-branch]` | Changes only the fields you pass. |
| `bibu pr diff ID` | Unified diff as raw text (even when piped). With `--json`: `{"pull_request", "diff"}`. |
| `bibu pr commits ID` / `bibu pr files ID` | Commits / changed files (`--limit N` or `--all`). |
| `bibu pr approve ID` / `unapprove ID` | Approve / withdraw approval. |
| `bibu pr request-changes ID` / `unrequest-changes ID` | Request / withdraw changes. |
| `bibu pr merge ID [--strategy S] [-m MSG] [--close-source-branch \| --keep-source-branch]` | Merge. Strategies: `merge_commit`, `squash`, `fast_forward`, `squash_fast_forward`, `rebase_fast_forward`, `rebase_merge`. |
| `bibu pr decline ID` | Decline. |

`merge` and `decline` are destructive: they ask `[y/N]` on a terminal, and **refuse to run without
a terminal unless `--yes` is passed** (exit 2, no request sent).

JSON shapes: `list` returns an array of pull requests; `view`, `edit`, `merge` and `decline` return
one pull request with `description`, `close_source_branch`, `merge_commit`, `participants`;
`create` returns `{"pull_requests": [...]}`; `approve` and friends return
`{"pull_request": ID, "action": "approved" | "approval_removed" | "changes_requested" | "change_request_removed"}`.

Token scopes (from Bitbucket's OpenAPI spec): read commands need `read:pullrequest:bitbucket`, write
commands `write:pullrequest:bitbucket`, and `diff` / `files` also `read:repository:bitbucket` (they
follow a redirect to the repository diff). `bibu pipeline` needs `read:pipeline:bitbucket`, `bibu branch` needs `read:repository:bitbucket`
(`write:repository:bitbucket` to create or delete), `bibu auth` needs `read:user:bitbucket`, and the `member`
commands plus name lookup in `pr reviewers add` need `read:workspace:bitbucket`. If a scope is missing
the error (exit 4) lists what Bitbucket says is required; treat that message as the source of truth.

### Comments

```sh
bibu pr comment list 12 [--unresolved] [--inline] [--file src/app.py] [--include-deleted] [--limit N | --all]
bibu pr comment add 12 "Looks good"                                    # general comment
bibu pr comment add 12 --file src/app.py "About this file"             # whole-file comment
bibu pr comment add 12 --file src/app.py --line 30 "Why this default?" # one line of the new version
bibu pr comment add 12 -f src/app.py -l 30 --end-line 35 "..."          # a range
bibu pr comment add 12 --file src/app.py --line 2 --old-side "..."     # a removed line (old version)
echo "long text" | bibu pr comment add 12 --file src/app.py --line 30  # text from stdin (or pass -)
bibu pr comment reply 12 COMMENT_ID "Agreed"
bibu pr comment edit 12 COMMENT_ID "New text"
bibu pr comment delete 12 COMMENT_ID            # asks; needs --yes without a terminal
bibu pr comment resolve 12 COMMENT_ID           # and: reopen
```

`list` shows threads oldest first, with replies indented under their parent. Deleted comments are
hidden unless `--include-deleted`. `--unresolved`, `--inline` and `--file` work on whole threads: a
reply is kept or dropped together with the comment that started its thread. JSON rows have `id`,
`parent_id`, `author`, `content`, `inline` (`path`, `line`, `end_line`, `side` = `new` | `old` | `file`),
`resolved`, `resolved_by`, `deleted`, `pending` and `url`. Resolving an already resolved thread is
exit 6, reopening one that is open is exit 5. Bitbucket accepts comments on any line, including lines
outside the diff.

### Reviewers

```sh
bibu pr reviewers list 12
bibu pr reviewers add 12 "Jane Doe" bob '{uuid}' me
bibu pr reviewers remove 12 bob
```

People are matched by uuid, account id, nickname or name (exact match first, then partial); `me` is
you. An ambiguous name is refused (exit 2) with the candidates listed; the author cannot review their
own PR (exit 6, from Bitbucket). Bitbucket replaces the whole reviewer list on update, so `add` and
`remove` read the current list first and send the merged one.

### Pipelines

```sh
bibu pipeline list [--branch main] [--status failed] [--limit N | --all]   # newest first
bibu pipeline view 42                # run + steps; 42 is the build number shown in Bitbucket (or a uuid)
bibu pipeline logs 42                # every step's log under a "==> name (status) <==" header
bibu pipeline logs 42 --step test    # one step: its name, 1-based number, or uuid
bibu pipeline logs 42 --json         # {"pipeline", "steps": [{"name", "status", "log"}]}
```

Statuses are one lowercase word: `pending`, `parsing`, `running`, `paused`, `halted`, `successful`,
`failed`, `stopped`, `error` (finished runs report their result, running ones their stage).
`logs` prints raw text even when piped, so `bibu pipeline logs 42 | grep -i error` works; a step that
has not started has no log yet and is reported as such instead of failing. Pipelines must be enabled
in the repository settings, and runs are started by pushing, not by bibu.

### Branches

```sh
bibu branch list [--name feature] [--limit N | --all]   # most recently updated first, default marked
bibu branch create feature/login                        # from the default branch
bibu branch create hotfix --from release/1.2            # from a branch, or a commit hash
bibu branch delete feature/login                        # asks; needs --yes without a terminal
```

`delete` refuses the repository's default branch even with `--yes` (exit 6). Branch names may contain
`/`, `#` and other characters; bibu encodes them for you.

### Tasks and members

```sh
bibu pr task list 12 [--unresolved]
bibu pr task add 12 "Rename the argument" [--comment COMMENT_ID]
bibu pr task resolve 12 TASK_ID                 # and: reopen
bibu member list [--workspace SLUG]
bibu member find jane
```

## Repository resolution

`--repo workspace/repo` (or a bitbucket.org URL), then `BIBU_REPO`, then the git `origin` remote.

```sh
bibu repo --repo acme/api   # {"workspace":"acme","slug":"api"}
```

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for branch and commit rules and how to add a command.
