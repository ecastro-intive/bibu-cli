---
name: bibu
description: Work with Bitbucket Cloud pull requests, inline review comments, reviewers, tasks, pipelines and branches through the `bibu` CLI. Use when the user asks to review, open, update, approve, merge or decline a Bitbucket pull request, read or answer review comments, see why a pipeline failed, list or create branches, or find a workspace member. Not for GitHub or GitLab.
---

# bibu: Bitbucket Cloud from the command line

`bibu` talks to Bitbucket Cloud. Everything below is a shell command you run yourself. It is built
to be driven by agents: stable JSON, stable exit codes, no prompts without a terminal.

## Before anything else

1. `bibu auth status --json` tells you whether credentials work. Exit code 3 means not logged in:
   ask the user to run `bibu auth login` themselves, or to export `BIBU_EMAIL` and `BIBU_TOKEN`.
   **Never ask the user to paste a token into the chat, and never print, log or commit one.**
2. bibu needs to know the repository. Inside a git checkout with a Bitbucket `origin` it works out
   `workspace/repo` by itself; otherwise pass `--repo workspace/repo` (or set `BIBU_REPO`).
   `bibu repo --json` shows what it resolved.
3. In scripts and agent runs use `BIBU_EMAIL` and `BIBU_TOKEN`. On macOS the keychain can show a
   permission dialog that hangs a non-interactive shell; the environment variables avoid it.

## Rules

- **Always pass `--json`** and parse the result. Output is JSON when piped anyway, but being
  explicit removes doubt. The exceptions are `bibu pr diff` and `bibu pipeline logs`, which print raw
  text (good for reading or `grep`); add `--json` only if you want them wrapped.
- **Decide on the exit code, never on message text.** Errors are one line of JSON on stderr:
  `{"error":{"code":"...","message":"...","hint":"..."}}`.
- **Destructive commands need the user's intent.** `pr merge`, `pr decline`, `pr comment delete` and
  `branch delete` refuse to run without a terminal unless `--yes` is given. Pass `--yes` only when
  the user asked for that exact action on that exact item. Never merge, decline or delete on your
  own initiative, and never to "clean up".
- **Do not guess identifiers.** Find pull request numbers with `pr list`, comment numbers with
  `pr comment list`, people with `member find`, pipeline numbers with `pipeline list`.
- **Discover, do not memorise.** `bibu schema` lists every command; `bibu schema pr comment add`
  gives the arguments and the exact JSON shape of that command's output.
- **Lists are paged.** The default is 25 items; add `--limit N` or `--all` when completeness matters.
- **Long or multi-line text goes through stdin**, not shell quoting: `comment add`, `comment reply`,
  `comment edit` and `task add` read stdin when the text argument is omitted or is `-`.
- Approving, requesting changes, resolving threads and merging are the user's decisions. Do them
  when asked; do not do them because a review "looks done".

## Exit codes

| Code | Meaning | What to do |
|---|---|---|
| 0 | success | continue |
| 2 | bad usage, missing `--yes`, or no repository | fix the command; do not retry unchanged |
| 3 | not logged in or token rejected | stop and ask the user to log in |
| 4 | token lacks a scope (the message names it) | tell the user which scope to add |
| 5 | not found (PR, comment, branch, pipeline, person) | re-list and check the id |
| 6 | refused: invalid input, already exists, already resolved, merge conflict | read the message, adjust |
| 7 | network, rate limit or server error | wait and retry once or twice |
| 1 | anything else | report it |

## Workflows

### Understand a pull request

```sh
bibu pr list --json
bibu pr list --author me --json
bibu pr view 12 --json
bibu pr files 12 --all --json
bibu pr commits 12 --json
bibu pr diff 12
```

`pr view` has the description, state, branches and every reviewer's state (`participants`).
`pr files` lists what changed with line counts, so read the diff file by file when it is large.

### Review with inline comments

```sh
bibu pr comment add 12 --file src/app.py --line 30 "This default hides the error. Return it instead."
bibu pr comment add 12 --file src/app.py --line 30 --end-line 36 "This whole block can be one query."
bibu pr comment add 12 --file src/app.py --line 2 --old-side "Why was this removed?"
bibu pr comment add 12 --file docs/notes.md "General note about this file."
bibu pr comment add 12 "Overall this looks good, two small things inline."
```

Line numbers refer to the **new** version of the file unless `--old-side` is given. Use the diff to
pick them. Keep each comment to one point. Pipe long text in instead of quoting it:

```sh
cat review.md | bibu pr comment add 12 --file src/app.py --line 30 --json
```

Finish with a decision only if the user asked for one:

```sh
bibu pr approve 12 --json
bibu pr request-changes 12 --json
```

### Work through review feedback

```sh
bibu pr comment list 12 --unresolved --json
bibu pr comment list 12 --file src/app.py --json
bibu pr comment reply 12 873864465 "Done, changed in the last commit."
bibu pr comment resolve 12 873864465 --json
bibu pr task list 12 --unresolved --json
bibu pr task resolve 12 73365243 --json
```

`comment list` returns threads oldest first; a reply has `parent_id`. `--unresolved` keeps whole
threads that are still open. Resolving twice returns exit 6 and is harmless.

### Open a pull request

```sh
bibu pr create --source feature/login --destination main --title "Add login" --description "Adds the login form." --json
bibu pr create -d main,develop --json
```

The source defaults to the current branch. The repository's default reviewers are added unless you
pass `--no-default-reviewers`. One pull request is created per destination. To change it later:

```sh
bibu pr edit 12 --title "Add login form" --json
bibu pr reviewers add 12 "Jane Doe" --json
bibu pr reviewers remove 12 bob --json
```

### Find out why a pipeline failed

```sh
bibu pipeline list --status failed --limit 5 --json
bibu pipeline view 42 --json
bibu pipeline logs 42 --step test
bibu pipeline logs 42
```

`pipeline view` shows which step failed. Read only that step's log first, then search it, for
example by piping `bibu pipeline logs 42 --step test` into `grep -i error`.

### Branches and people

```sh
bibu branch list --json
bibu branch list --name feature --json
bibu branch create feature/login --from main --json
bibu branch delete feature/old --yes --json
bibu member find jane --json
```

`branch delete` never deletes the default branch. `member find` shows the uuid you can pass to
`pr reviewers add` when a name is ambiguous (quote it, the braces matter to the shell).

### Merging

```sh
bibu pr merge 12 --strategy squash --close-source-branch --yes --json
bibu pr decline 12 --yes --json
```

Only when the user asked for it. Strategies: `merge_commit`, `squash`, `fast_forward`,
`squash_fast_forward`, `rebase_fast_forward`, `rebase_merge`. A merge conflict or failed merge check
is exit 6 with Bitbucket's reason in the message.

## Reading the output

- Lists are JSON arrays. Single results are objects. `pr create` returns `{"pull_requests": [...]}`.
- People look like `{"display_name", "uuid", "nickname", "account_id"}`.
- Timestamps are ISO 8601 strings. `url` fields link to the item in Bitbucket, worth quoting back to
  the user.
- A null field means "not applicable", for example `parent_id` on the first comment of a thread.

## Things that go wrong

- Exit 4 naming `read:pipeline:bitbucket`, `read:workspace:bitbucket` and similar: the API token was
  created without that scope. Tell the user; you cannot fix it.
- `member find` and `pr reviewers add` need the workspace read scope.
- A name that matches several people is refused with the candidates listed; use the uuid.
- The pull request author cannot be added as a reviewer (exit 6).
- Builds can only be started by pushing or from the Bitbucket UI; bibu reads pipelines, it does not
  start them.
