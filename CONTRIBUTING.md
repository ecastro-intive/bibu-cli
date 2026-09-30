# Contributing to bibu

## Setup

```sh
rustup toolchain install stable     # Rust 1.85 or newer
cargo build
cargo test
```

Before every commit:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same checks on macOS and Windows.

## Branches

```
<type>/<short-description>
```

- Types: `feat`, `fix`, `doc`, `chore`.
- At most 45 characters in total, lowercase kebab-case after the slash.
- Examples: `feat/pr-comments`, `fix/merge-exit-code`, `doc/contributing-guide`.

Branch off `main` and open a pull request. Do not push directly to `main`.

## Commits

```
<type>: <description>
```

- Types: `feat` (new capability), `fix` (bug), `doc` (docs only), `chore` (tooling, CI, deps, refactors, tests).
- The whole subject line is at most 50 characters.
- Lowercase, imperative ("add", not "added"), no trailing period.
- No footer or trailers (no `Co-Authored-By`).
- No em dashes, en dashes, or dashes used as punctuation. Reword instead.

Good: `feat: add inline pr comments`, `fix: map 429 to exit code 7`, `doc: document token scopes`.
Bad: `Feat: Added comments.`, `refactor: split module`, `fix: handle 404 — add hint`.

Claude Code users: the `commit-message` skill in `.claude/skills/commit-message/` enforces these
rules, and `CLAUDE.md` makes it run before every commit.

Pull request titles use the same format.

## What every change needs

- **Tests.** Each command has mocked-HTTP tests (`mockito` + `assert_cmd`) that assert the exact
  method, path, query, JSON body and `Authorization` header, plus error-path tests. Destructive
  commands need a test proving they refuse to run without `--yes` and without a terminal.
- **Docs.** Update the README (and, once generated, the command reference) in the same PR.
- **API accuracy.** Check endpoints, bodies and field names against Bitbucket's OpenAPI spec
  (`https://api.bitbucket.org/swagger.json`). Add the reference link in a doc comment on the
  function that calls the endpoint.
- **Output contract.** Results on stdout, errors on stderr, stable exit codes, JSON when piped.
  Never print the API token.

## Testing against real Bitbucket

Mocked tests are the safety net, but check new endpoints against a real repository too. Use a
throwaway repo and never a real project. Pass credentials with `BIBU_EMAIL` and `BIBU_TOKEN`
instead of the keychain while developing: macOS ties Keychain access to the exact binary, so every
rebuild of an unsigned dev binary pops a permission dialog (and hangs a non-interactive shell).

## Live tests

`tests/live.rs` runs bibu against a **real** Bitbucket repository. They are ignored by default and
refuse to run without `BIBU_TEST_ALLOW_WRITES=1`, because they create (and afterwards remove)
branches, pull requests, comments and tasks. Use a throwaway repository, never a real project.

```sh
BIBU_TEST_REPO=workspace/sandbox BIBU_TEST_EMAIL=you@company.com BIBU_TEST_TOKEN=... \
BIBU_TEST_ALLOW_WRITES=1 \
  cargo test --test live -- --ignored --test-threads=1
```

- `BIBU_TEST_REVIEWER` (name or uuid of another workspace member) enables the reviewer add/remove flow.
- `BIBU_TEST_ALLOW_MERGE=1` also runs the merge flow, which adds one commit to the default branch.
- Every JSON output is validated against the schema that `bibu schema` publishes for that command, so
  the suite also proves the documented shapes are the real ones.
- Cleanup runs even when an assertion fails. Commits are made through the REST API, so `git push`
  access to the sandbox is not needed.
- The pipeline test only reads existing runs (API commits do not start pipelines); it skips itself in
  a repository that has none.
- The `Live tests` workflow (Actions tab, run manually) does the same from CI once the
  `BIBU_TEST_*` repository secrets are set.

## Adding a command

1. Declare it in `src/cli/mod.rs` (clap derive). **Every command and every argument needs a doc
   comment**: a test fails otherwise, because the help text is the documentation.
2. Add the typed endpoint call in `src/api/<resource>.rs` and any models in `src/api/models/`.
3. Add the handler and its output type in `src/commands/`. Output types implement `Render` (a table
   view), `Serialize` and `schemars::JsonSchema` (the JSON view and its schema). **Every field needs a
   doc comment**: it becomes the field's description in `bibu schema` and in the docs, and a test
   fails if one is missing.
4. Wire it in `src/commands/mod.rs` and register its output type in `src/schema/outputs.rs`. A test
   fails if a runnable command has no registered output.
5. Write unit tests for pure logic and end-to-end tests in `tests/` (shared helpers live in
   `tests/common/mod.rs`).
6. Regenerate the reference docs: `UPDATE_DOCS=1 cargo test --test docs`, and commit the changes to
   `docs/`. CI fails when they are stale.
7. If agents should know about it, add an example to `skills/bibu/SKILL.md`. Every example there is
   parsed against the real CLI by `cargo test --test skill`.

## Pull requests

- One logical change per PR, with a title in the commit format.
- Describe what changed and how you verified it. Mention anything you could not test against
  real Bitbucket.
- CI must be green.
