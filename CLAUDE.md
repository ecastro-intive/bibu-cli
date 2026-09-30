# bibu-cli

Rust CLI for Bitbucket Cloud, built for people and AI agents. Targets Bitbucket Cloud only,
macOS and Windows. Output is a table in a terminal and JSON when piped or with `--json`;
errors are JSON on stderr with stable exit codes (see README).

## Commands

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass before a commit. Every command ships with mocked-HTTP tests and docs.

## Git rules (mandatory)

- **Before any `git commit` (including amend), any new branch, and any PR title, invoke the
  `commit-message` skill** (`.claude/skills/commit-message/SKILL.md`) and follow it.
- Commit: `<feat|fix|doc|chore>: <description>`, whole subject at most 50 characters, no footer, no dashes.
- Branch: `<type>/<short-description>`, at most 45 characters.
- Never push to `main` or force-push without the user's explicit go-ahead.

## Layout

See README and CONTRIBUTING.md. Endpoints and payloads must be checked against Bitbucket's
official OpenAPI spec (`https://api.bitbucket.org/swagger.json`), not memory.
