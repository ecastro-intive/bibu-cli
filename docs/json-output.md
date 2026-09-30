# Output, errors and exit codes

> Generated from the CLI definition. Do not edit by hand: run
> `UPDATE_DOCS=1 cargo test --test docs`.

## Output modes

- In a terminal, commands print a human-readable table or text.
- When stdout is **not** a terminal (piped, captured by a script or an agent), or when `--json`
is passed, commands print JSON. Pass `--json` explicitly when you depend on it.
- Results go to **stdout**; errors and prompts go to **stderr**. Nothing is written to stdout
when a command fails.
- `pr diff` and `pipeline logs` print raw text by default, even when piped, so they work with
`grep` and `less`; add `--json` to get them wrapped in JSON.
- `bibu schema` always prints JSON.
- bibu never asks a question without a terminal. Destructive commands (`pr merge`,
`pr decline`, `pr comment delete`, `branch delete`) need `--yes` in that case and exit 2
without it.

## Exit codes

| Code | `error.code` | Meaning |
|---|---|---|
| 0 | `none` | success |
| 1 | `other` | unexpected failure (I/O, unreadable credentials file, unexpected API response) |
| 2 | `usage` | bad arguments, unknown flag, missing confirmation (--yes), or no repository |
| 3 | `auth_invalid` | not logged in, or Bitbucket rejected the credentials (401) |
| 4 | `forbidden` | the API token lacks a scope or permission (403); the message names the scopes |
| 5 | `not_found` | the pull request, comment, branch, pipeline, person, ... does not exist (404) |
| 6 | `conflict` | Bitbucket refused the change: invalid input, already exists, merge conflict (400/409/422) |
| 7 | `network` | cannot reach Bitbucket, rate limited (429), or a server error (5xx); retrying later may work |

## Error format

On failure, stderr gets one line of JSON (or `error: ...` text in a terminal):

```json
{"error":{"code":"not_found","message":"no current reviewer matches \"bob\"","hint":null}}
```

| Field | Type | Description |
|---|---|---|
| `error` | object |  |
| `error.code` | string | Stable identifier of the error class; see the exit-code table. |
| `error.message` | string | Human readable explanation. Do not parse it. |
| `error.hint` | string or null | A suggested next step, when there is one. |

Branch on the exit code or `error.code`; never parse `error.message`.

## Credentials

Set `BIBU_EMAIL` and `BIBU_TOKEN` (both) to authenticate without the keychain, which is what scripts and agents should do. `BIBU_REPO` (or `--repo`) selects the repository, `BIBU_API_BASE` points at another API root (tests).
