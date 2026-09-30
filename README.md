# bibu

A Bitbucket Cloud CLI for humans and AI agents, written in Rust. Successor to bb-cli.

**Status:** under construction (milestone 3 of 7). Available so far: `bibu auth`, `bibu pr` (core commands) and `bibu repo`.

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

Token scopes: read commands need pull request read access (`read:pullrequest:bitbucket`) and
write commands need `write:pullrequest:bitbucket`. `diff` and `files` follow a redirect to the
repository diff, which may also need `read:repository:bitbucket`. If a scope is missing the error
(exit 4) lists what Bitbucket says is required; treat that message as the source of truth.

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
