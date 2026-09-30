# bibu command reference

> Generated from the CLI definition. Do not edit by hand: change the code, then run
> `UPDATE_DOCS=1 cargo test --test docs`. CI fails when this file is out of date.
> The same information is available as JSON from `bibu schema`.

## Global options

These are accepted by every command.

| Argument | Description | Notes |
|---|---|---|
| `--repo <REPO>` | Repository as `workspace/repo` or a bitbucket.org URL [default: git origin remote] |  |
| `--json` | Force JSON output (automatic when stdout is not a terminal) | default `false` |
| `-y`, `--yes` | Skip confirmation for destructive commands (required without a terminal) | default `false` |

## Commands

- [`bibu auth login`](#bibu-auth-login): Verify an Atlassian email + API token against Bitbucket, then store them
- [`bibu auth status`](#bibu-auth-status): Check that the active credentials work and show whose they are (exit 3 if not)
- [`bibu auth logout`](#bibu-auth-logout): Remove the stored credentials (BIBU_EMAIL / BIBU_TOKEN are not affected)
- [`bibu pr list`](#bibu-pr-list): List pull requests (open by default), newest first
- [`bibu pr view`](#bibu-pr-view): Show one pull request with its description and reviewers
- [`bibu pr create`](#bibu-pr-create): Create a pull request (one per destination branch)
- [`bibu pr edit`](#bibu-pr-edit): Change a pull request's title, description, destination, draft state or branch cleanup
- [`bibu pr diff`](#bibu-pr-diff): Print the pull request's unified diff (raw text unless --json)
- [`bibu pr commits`](#bibu-pr-commits): List the pull request's commits
- [`bibu pr files`](#bibu-pr-files): List the files changed by the pull request
- [`bibu pr approve`](#bibu-pr-approve): Approve a pull request
- [`bibu pr unapprove`](#bibu-pr-unapprove): Withdraw your approval
- [`bibu pr request-changes`](#bibu-pr-request-changes): Request changes on a pull request
- [`bibu pr unrequest-changes`](#bibu-pr-unrequest-changes): Withdraw your change request
- [`bibu pr merge`](#bibu-pr-merge): Merge a pull request (asks for confirmation; --yes skips it)
- [`bibu pr decline`](#bibu-pr-decline): Decline a pull request (asks for confirmation; --yes skips it)
- [`bibu pr comment list`](#bibu-pr-comment-list): List comments as threads, oldest first (deleted ones are hidden)
- [`bibu pr comment add`](#bibu-pr-comment-add): Add a comment: general, on a file, or on a line or line range
- [`bibu pr comment reply`](#bibu-pr-comment-reply): Reply to a comment
- [`bibu pr comment edit`](#bibu-pr-comment-edit): Change the text of one of your comments
- [`bibu pr comment delete`](#bibu-pr-comment-delete): Delete one of your comments (asks for confirmation; --yes skips it)
- [`bibu pr comment resolve`](#bibu-pr-comment-resolve): Mark a comment thread as resolved
- [`bibu pr comment reopen`](#bibu-pr-comment-reopen): Reopen a resolved comment thread
- [`bibu pr reviewers list`](#bibu-pr-reviewers-list): List reviewers and their review state
- [`bibu pr reviewers add`](#bibu-pr-reviewers-add): Add reviewers, keeping the existing ones
- [`bibu pr reviewers remove`](#bibu-pr-reviewers-remove): Remove reviewers, keeping the others
- [`bibu pr task list`](#bibu-pr-task-list): List a pull request's tasks
- [`bibu pr task add`](#bibu-pr-task-add): Add a task, optionally attached to a comment
- [`bibu pr task resolve`](#bibu-pr-task-resolve): Mark a task resolved
- [`bibu pr task reopen`](#bibu-pr-task-reopen): Mark a resolved task unresolved again
- [`bibu pipeline list`](#bibu-pipeline-list): List pipeline runs, newest first
- [`bibu pipeline view`](#bibu-pipeline-view): Show one run and its steps
- [`bibu pipeline logs`](#bibu-pipeline-logs): Print step logs (raw text unless --json)
- [`bibu branch list`](#bibu-branch-list): List branches, most recently updated first
- [`bibu branch create`](#bibu-branch-create): Create a branch from a branch or a commit
- [`bibu branch delete`](#bibu-branch-delete): Delete a branch (asks for confirmation; --yes skips it; never the default branch)
- [`bibu member list`](#bibu-member-list): List workspace members
- [`bibu member find`](#bibu-member-find): Find members by name, nickname, account id or uuid
- [`bibu schema`](#bibu-schema): Describe every command, argument and output as JSON (for agents and tools)
- [`bibu repo`](#bibu-repo): Show which repository bibu resolved (from --repo, BIBU_REPO or the git remote)

## `bibu auth login`

Verify an Atlassian email + API token against Bitbucket, then store them

```text
Verify an Atlassian email + API token against Bitbucket, then store them.

Create the token in Bitbucket: Personal settings > API tokens. Use your Atlassian account email (not a Bitbucket username). The token is stored in the OS keychain.

Non-interactive: echo "$TOKEN" | bibu auth login --email you@company.com --with-token
```

```text
bibu auth login [OPTIONS]
```

| Argument | Description | Notes |
|---|---|---|
| `--email <EMAIL>` | Atlassian account email (prompted for when omitted on a terminal) |  |
| `--with-token` | Read the API token from stdin instead of prompting | default `false` |

**Output** (the verified account and where the login was stored)

| Field | Type | Description |
|---|---|---|
| `email` | string | The Atlassian email that was verified. |
| `account` | object | The Bitbucket account the token belongs to. |
| `account.display_name` | string | Display name. |
| `account.uuid` | string | Bitbucket uuid, with braces. |
| `account.nickname` | string or null | Short handle, when the account has one. |
| `account.account_id` | string or null | Atlassian account id. |
| `stored_in` | "env" \\| "keychain" \\| "file" | Where the login was saved: `keychain` or `file`. |

## `bibu auth status`

Check that the active credentials work and show whose they are (exit 3 if not)

```text
bibu auth status [OPTIONS]
```

**Output** (the account behind the active credentials)

| Field | Type | Description |
|---|---|---|
| `authenticated` | boolean | Always true: an invalid or missing login is an error (exit 3), not `false` here. |
| `source` | "env" \\| "keychain" \\| "file" | Where the active credentials come from: `env`, `keychain` or `file`. |
| `email` | string | Email of the active credentials. |
| `account` | object | The Bitbucket account the token belongs to. |
| `account.display_name` | string | Display name. |
| `account.uuid` | string | Bitbucket uuid, with braces. |
| `account.nickname` | string or null | Short handle, when the account has one. |
| `account.account_id` | string or null | Atlassian account id. |

## `bibu auth logout`

Remove the stored credentials (BIBU_EMAIL / BIBU_TOKEN are not affected)

```text
bibu auth logout [OPTIONS]
```

**Output** (whether a stored login was removed)

| Field | Type | Description |
|---|---|---|
| `removed` | boolean | Whether a stored login was found and removed. |
| `env_credentials_still_set` | boolean | `BIBU_EMAIL` / `BIBU_TOKEN` are still set, so requests stay authenticated. |

## `bibu pr list`

List pull requests (open by default), newest first

```text
bibu pr list [OPTIONS]
```

| Argument | Description | Notes |
|---|---|---|
| `--state <STATE>` | Only pull requests in this state | one of `open`, `merged`, `declined`, `superseded`; default `open` |
| `--author <AUTHOR>` | Only pull requests by this author: nickname, display name, uuid, or `me` |  |
| `--source <SOURCE>` | Only pull requests from this source branch |  |
| `--destination <DESTINATION>` | Only pull requests into this destination branch |  |
| `--reviews` | Include reviewers and their approval state (one extra request per pull request) | default `false` |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of pull requests)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].id` | integer | Pull request number. |
| `[].[].title` | string | Title. |
| `[].[].state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `[].[].draft` | boolean | Whether it is a draft. |
| `[].[].author` | object or null | Who opened it. |
| `[].[].author.display_name` | string | Display name. |
| `[].[].author.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].author.nickname` | string or null | Short handle, when the account has one. |
| `[].[].author.account_id` | string or null | Atlassian account id. |
| `[].[].source_branch` | string | Branch the changes come from. |
| `[].[].destination_branch` | string | Branch the changes go into. |
| `[].[].comment_count` | integer | Number of comments. |
| `[].[].task_count` | integer | Number of open tasks. |
| `[].[].created_on` | string | ISO 8601 creation time. |
| `[].[].updated_on` | string | ISO 8601 time of the last update. |
| `[].[].url` | string | Link to the pull request in Bitbucket. |
| `[].[].participants` | array of object or null | Present for `view` and `list --reviews`. |
| `[].[].participants[].user` | object | The person. |
| `[].[].participants[].user.display_name` | string | Display name. |
| `[].[].participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `[].[].participants[].user.account_id` | string or null | Atlassian account id. |
| `[].[].participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `[].[].participants[].approved` | boolean | Whether they approved. |
| `[].[].participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |

## `bibu pr view`

Show one pull request with its description and reviewers

```text
bibu pr view [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (one pull request with description and participants)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Pull request number. |
| `title` | string | Title. |
| `state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `draft` | boolean | Whether it is a draft. |
| `author` | object or null | Who opened it. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `source_branch` | string | Branch the changes come from. |
| `destination_branch` | string | Branch the changes go into. |
| `comment_count` | integer | Number of comments. |
| `task_count` | integer | Number of open tasks. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last update. |
| `url` | string | Link to the pull request in Bitbucket. |
| `participants` | array of object or null | Present for `view` and `list --reviews`. |
| `participants[].user` | object | The person. |
| `participants[].user.display_name` | string | Display name. |
| `participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `participants[].user.account_id` | string or null | Atlassian account id. |
| `participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `participants[].approved` | boolean | Whether they approved. |
| `participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |
| `description` | string | Description (markdown). |
| `close_source_branch` | boolean | Whether the source branch is deleted when the pull request is merged. |
| `merge_commit` | string or null | Hash of the merge commit, once merged. |
| `reason` | string or null | Why it was declined, when it was. |

## `bibu pr create`

Create a pull request (one per destination branch)

```text
Create a pull request from the source branch into each destination.

The source defaults to the current git branch. Several destinations (comma-separated) create one pull request each. The repository's default reviewers are added, minus you, unless --no-default-reviewers is given.
```

```text
bibu pr create [OPTIONS] --destination <DESTINATION>
```

| Argument | Description | Notes |
|---|---|---|
| `-d`, `--destination <DESTINATION>` | Destination branch(es), comma-separated | required; repeatable |
| `-s`, `--source <SOURCE>` | Source branch [default: current git branch] |  |
| `-t`, `--title <TITLE>` | Title [default: "Merge <source> into <destination>"] |  |
| `-m`, `--description <DESCRIPTION>` | Description (markdown) |  |
| `--draft` | Create as a draft | default `false` |
| `--close-source-branch` | Delete the source branch when the pull request is merged | default `false` |
| `--no-default-reviewers` | Do not add the repository's default reviewers | default `false` |

**Output** (the pull requests that were created)

| Field | Type | Description |
|---|---|---|
| `pull_requests` | array of object | The pull requests that were created, one per destination branch. |
| `pull_requests[].id` | integer | Pull request number. |
| `pull_requests[].title` | string | Title. |
| `pull_requests[].state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `pull_requests[].draft` | boolean | Whether it is a draft. |
| `pull_requests[].author` | object or null | Who opened it. |
| `pull_requests[].author.display_name` | string | Display name. |
| `pull_requests[].author.uuid` | string | Bitbucket uuid, with braces. |
| `pull_requests[].author.nickname` | string or null | Short handle, when the account has one. |
| `pull_requests[].author.account_id` | string or null | Atlassian account id. |
| `pull_requests[].source_branch` | string | Branch the changes come from. |
| `pull_requests[].destination_branch` | string | Branch the changes go into. |
| `pull_requests[].comment_count` | integer | Number of comments. |
| `pull_requests[].task_count` | integer | Number of open tasks. |
| `pull_requests[].created_on` | string | ISO 8601 creation time. |
| `pull_requests[].updated_on` | string | ISO 8601 time of the last update. |
| `pull_requests[].url` | string | Link to the pull request in Bitbucket. |
| `pull_requests[].participants` | array of object or null | Present for `view` and `list --reviews`. |
| `pull_requests[].participants[].user` | object | The person. |
| `pull_requests[].participants[].user.display_name` | string | Display name. |
| `pull_requests[].participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `pull_requests[].participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `pull_requests[].participants[].user.account_id` | string or null | Atlassian account id. |
| `pull_requests[].participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `pull_requests[].participants[].approved` | boolean | Whether they approved. |
| `pull_requests[].participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |

## `bibu pr edit`

Change a pull request's title, description, destination, draft state or branch cleanup

```text
bibu pr edit [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `-t`, `--title <TITLE>` | New title |  |
| `-m`, `--description <DESCRIPTION>` | New description; an empty string clears it |  |
| `-d`, `--destination <DESTINATION>` | New destination branch |  |
| `--draft` | Convert to a draft | default `false` |
| `--ready` | Mark a draft as ready for review | default `false` |
| `--close-source-branch` | Delete the source branch when merged | default `false` |
| `--keep-source-branch` | Keep the source branch when merged | default `false` |

**Output** (one pull request with description and participants)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Pull request number. |
| `title` | string | Title. |
| `state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `draft` | boolean | Whether it is a draft. |
| `author` | object or null | Who opened it. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `source_branch` | string | Branch the changes come from. |
| `destination_branch` | string | Branch the changes go into. |
| `comment_count` | integer | Number of comments. |
| `task_count` | integer | Number of open tasks. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last update. |
| `url` | string | Link to the pull request in Bitbucket. |
| `participants` | array of object or null | Present for `view` and `list --reviews`. |
| `participants[].user` | object | The person. |
| `participants[].user.display_name` | string | Display name. |
| `participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `participants[].user.account_id` | string or null | Atlassian account id. |
| `participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `participants[].approved` | boolean | Whether they approved. |
| `participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |
| `description` | string | Description (markdown). |
| `close_source_branch` | boolean | Whether the source branch is deleted when the pull request is merged. |
| `merge_commit` | string or null | Hash of the merge commit, once merged. |
| `reason` | string or null | Why it was declined, when it was. |

## `bibu pr diff`

Print the pull request's unified diff (raw text unless --json)

```text
bibu pr diff [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the unified diff (raw text by default); raw text unless `--json` is given)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `diff` | string | The unified diff. |

## `bibu pr commits`

List the pull request's commits

```text
bibu pr commits [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of commits)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].hash` | string | Full commit hash. |
| `[].[].author` | string | Author's display name. |
| `[].[].date` | string | ISO 8601 commit time. |
| `[].[].subject` | string | First line of the commit message. |
| `[].[].url` | string | Link to the commit in Bitbucket. |

## `bibu pr files`

List the files changed by the pull request

```text
bibu pr files [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of changed files)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].path` | string | Path of the changed file (the old path for removed files). |
| `[].[].status` | string | `added`, `modified`, `removed` or `renamed`. |
| `[].[].lines_added` | integer | Lines added. |
| `[].[].lines_removed` | integer | Lines removed. |

## `bibu pr approve`

Approve a pull request

```text
bibu pr approve [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the pull request and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `action` | string | What happened: `approved`, `approval_removed`, `changes_requested` or `change_request_removed`. |

## `bibu pr unapprove`

Withdraw your approval

```text
bibu pr unapprove [OPTIONS] <ID>
```

Aliases: `no-approve`

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the pull request and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `action` | string | What happened: `approved`, `approval_removed`, `changes_requested` or `change_request_removed`. |

## `bibu pr request-changes`

Request changes on a pull request

```text
bibu pr request-changes [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the pull request and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `action` | string | What happened: `approved`, `approval_removed`, `changes_requested` or `change_request_removed`. |

## `bibu pr unrequest-changes`

Withdraw your change request

```text
bibu pr unrequest-changes [OPTIONS] <ID>
```

Aliases: `no-request-changes`

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the pull request and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `action` | string | What happened: `approved`, `approval_removed`, `changes_requested` or `change_request_removed`. |

## `bibu pr merge`

Merge a pull request (asks for confirmation; --yes skips it)

```text
bibu pr merge [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `--strategy <STRATEGY>` | Merge strategy [default: the repository's default] | one of `merge_commit`, `squash`, `fast_forward`, `squash_fast_forward`, `rebase_fast_forward`, `rebase_merge` |
| `-m`, `--message <MESSAGE>` | Commit message for the merge |  |
| `--close-source-branch` | Delete the source branch after merging | default `false` |
| `--keep-source-branch` | Keep the source branch after merging | default `false` |

**Output** (one pull request with description and participants)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Pull request number. |
| `title` | string | Title. |
| `state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `draft` | boolean | Whether it is a draft. |
| `author` | object or null | Who opened it. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `source_branch` | string | Branch the changes come from. |
| `destination_branch` | string | Branch the changes go into. |
| `comment_count` | integer | Number of comments. |
| `task_count` | integer | Number of open tasks. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last update. |
| `url` | string | Link to the pull request in Bitbucket. |
| `participants` | array of object or null | Present for `view` and `list --reviews`. |
| `participants[].user` | object | The person. |
| `participants[].user.display_name` | string | Display name. |
| `participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `participants[].user.account_id` | string or null | Atlassian account id. |
| `participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `participants[].approved` | boolean | Whether they approved. |
| `participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |
| `description` | string | Description (markdown). |
| `close_source_branch` | boolean | Whether the source branch is deleted when the pull request is merged. |
| `merge_commit` | string or null | Hash of the merge commit, once merged. |
| `reason` | string or null | Why it was declined, when it was. |

## `bibu pr decline`

Decline a pull request (asks for confirmation; --yes skips it)

```text
bibu pr decline [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (one pull request with description and participants)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Pull request number. |
| `title` | string | Title. |
| `state` | string | `OPEN`, `MERGED`, `DECLINED` or `SUPERSEDED`. |
| `draft` | boolean | Whether it is a draft. |
| `author` | object or null | Who opened it. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `source_branch` | string | Branch the changes come from. |
| `destination_branch` | string | Branch the changes go into. |
| `comment_count` | integer | Number of comments. |
| `task_count` | integer | Number of open tasks. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last update. |
| `url` | string | Link to the pull request in Bitbucket. |
| `participants` | array of object or null | Present for `view` and `list --reviews`. |
| `participants[].user` | object | The person. |
| `participants[].user.display_name` | string | Display name. |
| `participants[].user.uuid` | string | Bitbucket uuid, with braces. |
| `participants[].user.nickname` | string or null | Short handle, when the account has one. |
| `participants[].user.account_id` | string or null | Atlassian account id. |
| `participants[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `participants[].approved` | boolean | Whether they approved. |
| `participants[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |
| `description` | string | Description (markdown). |
| `close_source_branch` | boolean | Whether the source branch is deleted when the pull request is merged. |
| `merge_commit` | string or null | Hash of the merge commit, once merged. |
| `reason` | string or null | Why it was declined, when it was. |

## `bibu pr comment list`

List comments as threads, oldest first (deleted ones are hidden)

```text
bibu pr comment list [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `--unresolved` | Only threads whose first comment is not resolved | default `false` |
| `--inline` | Only inline comments (anchored to a file) | default `false` |
| `--file <FILE>` | Only threads anchored to this file path (implies --inline) |  |
| `--include-deleted` | Also show deleted comments | default `false` |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of comments, oldest first)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].id` | integer | Comment number. |
| `[].[].parent_id` | integer or null | Number of the comment this replies to; null for the start of a thread. |
| `[].[].author` | object or null | Who wrote the comment. |
| `[].[].author.display_name` | string | Display name. |
| `[].[].author.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].author.nickname` | string or null | Short handle, when the account has one. |
| `[].[].author.account_id` | string or null | Atlassian account id. |
| `[].[].created_on` | string | ISO 8601 creation time. |
| `[].[].updated_on` | string | ISO 8601 time of the last edit. |
| `[].[].content` | string | Comment text (markdown). |
| `[].[].inline` | object or null | Where the comment is anchored; null for a general comment. |
| `[].[].inline.path` | string | File path, relative to the repository root. |
| `[].[].inline.line` | integer or null | First line (the only line for a single-line comment); null for a whole-file comment. |
| `[].[].inline.end_line` | integer or null | Last line when the comment spans a range. |
| `[].[].inline.side` | string | Which version of the file the lines refer to: `new`, `old`, or `file` for a whole-file comment. |
| `[].[].resolved` | boolean | Whether the thread is resolved (recorded on the thread's first comment). |
| `[].[].resolved_by` | object or null | Who resolved the thread, when Bitbucket reports it. |
| `[].[].resolved_by.display_name` | string | Display name. |
| `[].[].resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `[].[].resolved_by.account_id` | string or null | Atlassian account id. |
| `[].[].deleted` | boolean | Deleted comments are hidden from `list` unless `--include-deleted`. |
| `[].[].pending` | boolean | A draft comment of an unfinished review. |
| `[].[].url` | string | Link to the comment in Bitbucket. |

## `bibu pr comment add`

Add a comment: general, on a file, or on a line or line range

```text
Add a comment to a pull request.

Without --file the comment is general. With --file it is attached to that file; add --line to attach it to a line of the new version, --end-line for a range, and --old-side to anchor to the old version (for removed lines). The text is the last argument; pass - or omit it to read the text from stdin.

Examples:
  bibu pr comment add 12 "Looks good overall"
  bibu pr comment add 12 --file src/app.py --line 30 "Why this default?"
  echo "long text" | bibu pr comment add 12 --file src/app.py --line 30 --end-line 35
```

```text
bibu pr comment add [OPTIONS] <ID> [TEXT]
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<TEXT>` | Comment text (markdown); `-` or omitted reads stdin |  |
| `-f`, `--file <FILE>` | File path, relative to the repository root |  |
| `-l`, `--line <LINE>` | Line number (requires --file) |  |
| `--end-line <END_LINE>` | Last line of a range that starts at --line |  |
| `--old-side` | Anchor to the old version of the file instead of the new one | default `false` |

**Output** (the comment)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Comment number. |
| `parent_id` | integer or null | Number of the comment this replies to; null for the start of a thread. |
| `author` | object or null | Who wrote the comment. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last edit. |
| `content` | string | Comment text (markdown). |
| `inline` | object or null | Where the comment is anchored; null for a general comment. |
| `inline.path` | string | File path, relative to the repository root. |
| `inline.line` | integer or null | First line (the only line for a single-line comment); null for a whole-file comment. |
| `inline.end_line` | integer or null | Last line when the comment spans a range. |
| `inline.side` | string | Which version of the file the lines refer to: `new`, `old`, or `file` for a whole-file comment. |
| `resolved` | boolean | Whether the thread is resolved (recorded on the thread's first comment). |
| `resolved_by` | object or null | Who resolved the thread, when Bitbucket reports it. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `deleted` | boolean | Deleted comments are hidden from `list` unless `--include-deleted`. |
| `pending` | boolean | A draft comment of an unfinished review. |
| `url` | string | Link to the comment in Bitbucket. |

## `bibu pr comment reply`

Reply to a comment

```text
bibu pr comment reply [OPTIONS] <ID> <COMMENT_ID> [TEXT]
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<COMMENT_ID>` | Number of the comment to reply to | required |
| `<TEXT>` | Reply text (markdown); `-` or omitted reads stdin |  |

**Output** (the comment)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Comment number. |
| `parent_id` | integer or null | Number of the comment this replies to; null for the start of a thread. |
| `author` | object or null | Who wrote the comment. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last edit. |
| `content` | string | Comment text (markdown). |
| `inline` | object or null | Where the comment is anchored; null for a general comment. |
| `inline.path` | string | File path, relative to the repository root. |
| `inline.line` | integer or null | First line (the only line for a single-line comment); null for a whole-file comment. |
| `inline.end_line` | integer or null | Last line when the comment spans a range. |
| `inline.side` | string | Which version of the file the lines refer to: `new`, `old`, or `file` for a whole-file comment. |
| `resolved` | boolean | Whether the thread is resolved (recorded on the thread's first comment). |
| `resolved_by` | object or null | Who resolved the thread, when Bitbucket reports it. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `deleted` | boolean | Deleted comments are hidden from `list` unless `--include-deleted`. |
| `pending` | boolean | A draft comment of an unfinished review. |
| `url` | string | Link to the comment in Bitbucket. |

## `bibu pr comment edit`

Change the text of one of your comments

```text
bibu pr comment edit [OPTIONS] <ID> <COMMENT_ID> [TEXT]
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<COMMENT_ID>` | Comment number | required |
| `<TEXT>` | New text (markdown); `-` or omitted reads stdin |  |

**Output** (the comment)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Comment number. |
| `parent_id` | integer or null | Number of the comment this replies to; null for the start of a thread. |
| `author` | object or null | Who wrote the comment. |
| `author.display_name` | string | Display name. |
| `author.uuid` | string | Bitbucket uuid, with braces. |
| `author.nickname` | string or null | Short handle, when the account has one. |
| `author.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `updated_on` | string | ISO 8601 time of the last edit. |
| `content` | string | Comment text (markdown). |
| `inline` | object or null | Where the comment is anchored; null for a general comment. |
| `inline.path` | string | File path, relative to the repository root. |
| `inline.line` | integer or null | First line (the only line for a single-line comment); null for a whole-file comment. |
| `inline.end_line` | integer or null | Last line when the comment spans a range. |
| `inline.side` | string | Which version of the file the lines refer to: `new`, `old`, or `file` for a whole-file comment. |
| `resolved` | boolean | Whether the thread is resolved (recorded on the thread's first comment). |
| `resolved_by` | object or null | Who resolved the thread, when Bitbucket reports it. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `deleted` | boolean | Deleted comments are hidden from `list` unless `--include-deleted`. |
| `pending` | boolean | A draft comment of an unfinished review. |
| `url` | string | Link to the comment in Bitbucket. |

## `bibu pr comment delete`

Delete one of your comments (asks for confirmation; --yes skips it)

```text
bibu pr comment delete [OPTIONS] <ID> <COMMENT_ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<COMMENT_ID>` | Comment number | required |

**Output** (the comment and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `comment` | integer | Comment number. |
| `action` | string | What happened: `deleted`, `resolved` or `reopened`. |

## `bibu pr comment resolve`

Mark a comment thread as resolved

```text
bibu pr comment resolve [OPTIONS] <ID> <COMMENT_ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<COMMENT_ID>` | Comment number | required |

**Output** (the comment and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `comment` | integer | Comment number. |
| `action` | string | What happened: `deleted`, `resolved` or `reopened`. |

## `bibu pr comment reopen`

Reopen a resolved comment thread

```text
bibu pr comment reopen [OPTIONS] <ID> <COMMENT_ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<COMMENT_ID>` | Comment number | required |

**Output** (the comment and what happened)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `comment` | integer | Comment number. |
| `action` | string | What happened: `deleted`, `resolved` or `reopened`. |

## `bibu pr reviewers list`

List reviewers and their review state

```text
bibu pr reviewers list [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |

**Output** (the pull request's reviewers and their review state)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `reviewers` | array of object | Current reviewers and their review state. |
| `reviewers[].user` | object | The person. |
| `reviewers[].user.display_name` | string | Display name. |
| `reviewers[].user.uuid` | string | Bitbucket uuid, with braces. |
| `reviewers[].user.nickname` | string or null | Short handle, when the account has one. |
| `reviewers[].user.account_id` | string or null | Atlassian account id. |
| `reviewers[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `reviewers[].approved` | boolean | Whether they approved. |
| `reviewers[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |

## `bibu pr reviewers add`

Add reviewers, keeping the existing ones

```text
Add reviewers, keeping the existing ones.

Each person is a name, nickname, account id, `{uuid}`, or `me`; names are looked up in the workspace members and must match exactly one person. Bitbucket replaces the whole list on update, so bibu reads the current reviewers first and sends the merged list.
```

```text
bibu pr reviewers add [OPTIONS] <ID> <USERS>...
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<USERS>` | People to add | required; repeatable |

**Output** (the pull request's reviewers and their review state)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `reviewers` | array of object | Current reviewers and their review state. |
| `reviewers[].user` | object | The person. |
| `reviewers[].user.display_name` | string | Display name. |
| `reviewers[].user.uuid` | string | Bitbucket uuid, with braces. |
| `reviewers[].user.nickname` | string or null | Short handle, when the account has one. |
| `reviewers[].user.account_id` | string or null | Atlassian account id. |
| `reviewers[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `reviewers[].approved` | boolean | Whether they approved. |
| `reviewers[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |

## `bibu pr reviewers remove`

Remove reviewers, keeping the others

```text
bibu pr reviewers remove [OPTIONS] <ID> <USERS>...
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<USERS>` | People to remove (name, nickname, account id or `{uuid}`) | required; repeatable |

**Output** (the pull request's reviewers and their review state)

| Field | Type | Description |
|---|---|---|
| `pull_request` | integer | Pull request number. |
| `reviewers` | array of object | Current reviewers and their review state. |
| `reviewers[].user` | object | The person. |
| `reviewers[].user.display_name` | string | Display name. |
| `reviewers[].user.uuid` | string | Bitbucket uuid, with braces. |
| `reviewers[].user.nickname` | string or null | Short handle, when the account has one. |
| `reviewers[].user.account_id` | string or null | Atlassian account id. |
| `reviewers[].role` | string | `REVIEWER`, or `PARTICIPANT` for someone who only commented or approved. |
| `reviewers[].approved` | boolean | Whether they approved. |
| `reviewers[].state` | string or null | `approved`, `changes_requested`, or null while pending / commented only. |

## `bibu pr task list`

List a pull request's tasks

```text
bibu pr task list [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `--unresolved` | Only tasks that are not resolved | default `false` |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of tasks)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].id` | integer | Task number. |
| `[].[].state` | string | `RESOLVED` or `UNRESOLVED`. |
| `[].[].content` | string | Task text. |
| `[].[].creator` | object or null | Who created the task. |
| `[].[].creator.display_name` | string | Display name. |
| `[].[].creator.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].creator.nickname` | string or null | Short handle, when the account has one. |
| `[].[].creator.account_id` | string or null | Atlassian account id. |
| `[].[].created_on` | string | ISO 8601 creation time. |
| `[].[].resolved_by` | object or null | Who resolved it, when resolved. |
| `[].[].resolved_by.display_name` | string | Display name. |
| `[].[].resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `[].[].resolved_by.account_id` | string or null | Atlassian account id. |
| `[].[].resolved_on` | string or null | ISO 8601 time it was resolved. |
| `[].[].comment_id` | integer or null | The comment this task is attached to, if any. |

## `bibu pr task add`

Add a task, optionally attached to a comment

```text
bibu pr task add [OPTIONS] <ID> [TEXT]
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<TEXT>` | Task text; `-` or omitted reads stdin |  |
| `--comment <COMMENT>` | Attach the task to this comment |  |

**Output** (the task)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Task number. |
| `state` | string | `RESOLVED` or `UNRESOLVED`. |
| `content` | string | Task text. |
| `creator` | object or null | Who created the task. |
| `creator.display_name` | string | Display name. |
| `creator.uuid` | string | Bitbucket uuid, with braces. |
| `creator.nickname` | string or null | Short handle, when the account has one. |
| `creator.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `resolved_by` | object or null | Who resolved it, when resolved. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `resolved_on` | string or null | ISO 8601 time it was resolved. |
| `comment_id` | integer or null | The comment this task is attached to, if any. |

## `bibu pr task resolve`

Mark a task resolved

```text
bibu pr task resolve [OPTIONS] <ID> <TASK_ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<TASK_ID>` | Task number | required |

**Output** (the task)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Task number. |
| `state` | string | `RESOLVED` or `UNRESOLVED`. |
| `content` | string | Task text. |
| `creator` | object or null | Who created the task. |
| `creator.display_name` | string | Display name. |
| `creator.uuid` | string | Bitbucket uuid, with braces. |
| `creator.nickname` | string or null | Short handle, when the account has one. |
| `creator.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `resolved_by` | object or null | Who resolved it, when resolved. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `resolved_on` | string or null | ISO 8601 time it was resolved. |
| `comment_id` | integer or null | The comment this task is attached to, if any. |

## `bibu pr task reopen`

Mark a resolved task unresolved again

```text
bibu pr task reopen [OPTIONS] <ID> <TASK_ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Pull request number | required |
| `<TASK_ID>` | Task number | required |

**Output** (the task)

| Field | Type | Description |
|---|---|---|
| `id` | integer | Task number. |
| `state` | string | `RESOLVED` or `UNRESOLVED`. |
| `content` | string | Task text. |
| `creator` | object or null | Who created the task. |
| `creator.display_name` | string | Display name. |
| `creator.uuid` | string | Bitbucket uuid, with braces. |
| `creator.nickname` | string or null | Short handle, when the account has one. |
| `creator.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 creation time. |
| `resolved_by` | object or null | Who resolved it, when resolved. |
| `resolved_by.display_name` | string | Display name. |
| `resolved_by.uuid` | string | Bitbucket uuid, with braces. |
| `resolved_by.nickname` | string or null | Short handle, when the account has one. |
| `resolved_by.account_id` | string or null | Atlassian account id. |
| `resolved_on` | string or null | ISO 8601 time it was resolved. |
| `comment_id` | integer or null | The comment this task is attached to, if any. |

## `bibu pipeline list`

List pipeline runs, newest first

```text
bibu pipeline list [OPTIONS]
```

| Argument | Description | Notes |
|---|---|---|
| `--branch <BRANCH>` | Only runs of this branch |  |
| `--status <STATUS>` | Only runs with this status | one of `pending`, `parsing`, `running`, `paused`, `halted`, `successful`, `failed`, `stopped`, `error` |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of pipeline runs, newest first)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].uuid` | string | Pipeline uuid, with braces. |
| `[].[].build_number` | integer | Run number, as shown in Bitbucket. |
| `[].[].status` | string | `successful`, `failed`, `stopped`, `error`, `running`, `paused`, `pending`, ... |
| `[].[].branch` | string | Branch (or tag) the run is for. |
| `[].[].ref_type` | string | `branch`, `tag`, ... |
| `[].[].commit` | string or null | Full hash of the commit that was built. |
| `[].[].trigger` | string or null | `PUSH`, `MANUAL`, `SCHEDULED`, ... |
| `[].[].creator` | object or null | Who triggered the run. |
| `[].[].creator.display_name` | string | Display name. |
| `[].[].creator.uuid` | string | Bitbucket uuid, with braces. |
| `[].[].creator.nickname` | string or null | Short handle, when the account has one. |
| `[].[].creator.account_id` | string or null | Atlassian account id. |
| `[].[].created_on` | string | ISO 8601 time the run was created. |
| `[].[].completed_on` | string or null | ISO 8601 time the run finished; null while it is running. |
| `[].[].duration_seconds` | integer or null | Build time in seconds; null until known. |
| `[].[].url` | string | Link to the run in Bitbucket. |

## `bibu pipeline view`

Show one run and its steps

```text
Show one pipeline run and its steps.

The run is a build number (as in the Bitbucket UI, e.g. 42) or a uuid.
```

```text
bibu pipeline view [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Build number or uuid | required |

**Output** (one pipeline run with its steps)

| Field | Type | Description |
|---|---|---|
| `uuid` | string | Pipeline uuid, with braces. |
| `build_number` | integer | Run number, as shown in Bitbucket. |
| `status` | string | `successful`, `failed`, `stopped`, `error`, `running`, `paused`, `pending`, ... |
| `branch` | string | Branch (or tag) the run is for. |
| `ref_type` | string | `branch`, `tag`, ... |
| `commit` | string or null | Full hash of the commit that was built. |
| `trigger` | string or null | `PUSH`, `MANUAL`, `SCHEDULED`, ... |
| `creator` | object or null | Who triggered the run. |
| `creator.display_name` | string | Display name. |
| `creator.uuid` | string | Bitbucket uuid, with braces. |
| `creator.nickname` | string or null | Short handle, when the account has one. |
| `creator.account_id` | string or null | Atlassian account id. |
| `created_on` | string | ISO 8601 time the run was created. |
| `completed_on` | string or null | ISO 8601 time the run finished; null while it is running. |
| `duration_seconds` | integer or null | Build time in seconds; null until known. |
| `url` | string | Link to the run in Bitbucket. |
| `steps` | array of object | Steps in execution order. |
| `steps[].uuid` | string | Step uuid, with braces. |
| `steps[].name` | string | Step name from bitbucket-pipelines.yml. |
| `steps[].status` | string | Same vocabulary as the run status. |
| `steps[].started_on` | string or null | ISO 8601 time the step started; null if it has not. |
| `steps[].completed_on` | string or null | ISO 8601 time the step finished; null while running. |
| `steps[].duration_seconds` | integer or null | Step time in seconds; null until known. |

## `bibu pipeline logs`

Print step logs (raw text unless --json)

```text
Print the logs of a pipeline run.

Without --step every step's log is printed under a header; with --step only that step. A step is its name, its 1-based position, or its uuid. Output is raw text even when piped so it can go through grep; add --json for {pipeline, steps: [{name, status, log}]}.
```

```text
bibu pipeline logs [OPTIONS] <ID>
```

| Argument | Description | Notes |
|---|---|---|
| `<ID>` | Build number or uuid | required |
| `--step <STEP>` | Only this step: name, 1-based number, or uuid |  |

**Output** (the step logs (raw text by default); raw text unless `--json` is given)

| Field | Type | Description |
|---|---|---|
| `pipeline` | integer | Build number of the run. |
| `steps` | array of object | Logs of the selected steps, in order. |
| `steps[].uuid` | string | Step uuid, with braces. |
| `steps[].name` | string | Step name. |
| `steps[].status` | string | Step status, same vocabulary as the run status. |
| `steps[].log` | string or null | `None` when Bitbucket has no log for the step yet (for example it has not started). |

## `bibu branch list`

List branches, most recently updated first

```text
bibu branch list [OPTIONS]
```

| Argument | Description | Notes |
|---|---|---|
| `--name <NAME>` | Only branches whose name contains this text (case-insensitive) |  |
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of branches)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].name` | string | Branch name. |
| `[].[].hash` | string | Commit at the tip of the branch. |
| `[].[].date` | string | ISO 8601 time of the tip commit. |
| `[].[].author` | string | Author of the tip commit. |
| `[].[].message` | string | First line of the tip commit's message. |
| `[].[].default` | boolean | Whether this is the repository's default branch. |

## `bibu branch create`

Create a branch from a branch or a commit

```text
bibu branch create [OPTIONS] <NAME>
```

| Argument | Description | Notes |
|---|---|---|
| `<NAME>` | Name of the new branch | required |
| `--from <FROM>` | Branch name or commit hash to start from [default: the repository's default branch] |  |

**Output** (the new branch)

| Field | Type | Description |
|---|---|---|
| `name` | string | Branch name. |
| `hash` | string | Commit at the tip of the branch. |
| `date` | string | ISO 8601 time of the tip commit. |
| `author` | string | Author of the tip commit. |
| `message` | string | First line of the tip commit's message. |
| `default` | boolean | Whether this is the repository's default branch. |

## `bibu branch delete`

Delete a branch (asks for confirmation; --yes skips it; never the default branch)

```text
bibu branch delete [OPTIONS] <NAME>
```

| Argument | Description | Notes |
|---|---|---|
| `<NAME>` | Branch name | required |

**Output** (the branch and what happened)

| Field | Type | Description |
|---|---|---|
| `branch` | string | Branch name. |
| `action` | string | What happened: `deleted`. |

## `bibu member list`

List workspace members

```text
bibu member list [OPTIONS]
```

| Argument | Description | Notes |
|---|---|---|
| `--limit <LIMIT>` | Maximum number of items to return | default `25` |
| `--all` | Return every item, following all pages | default `false` |

**Output** (array of workspace members)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].display_name` | string | Display name. |
| `[].[].uuid` | string | Bitbucket uuid, with braces. |
| `[].[].nickname` | string or null | Short handle, when the account has one. |
| `[].[].account_id` | string or null | Atlassian account id. |

## `bibu member find`

Find members by name, nickname, account id or uuid

```text
bibu member find [OPTIONS] <QUERY>
```

| Argument | Description | Notes |
|---|---|---|
| `<QUERY>` | Text to look for (case-insensitive) | required |

**Output** (array of workspace members)

The value is an array; each element has:

| Field | Type | Description |
|---|---|---|
| `[].[].display_name` | string | Display name. |
| `[].[].uuid` | string | Bitbucket uuid, with braces. |
| `[].[].nickname` | string or null | Short handle, when the account has one. |
| `[].[].account_id` | string or null | Atlassian account id. |

## `bibu schema`

Describe every command, argument and output as JSON (for agents and tools)

```text
Describe the CLI as JSON: commands, arguments, and for every command the JSON Schema of what it prints, plus the exit codes and the error format.

With no argument you get a compact index of every command (no output schemas). Name a command to get everything about it, including the schema of its output; naming a group such as `pr` lists its commands compactly. Use --full to include every output schema. This command always prints JSON.

Examples:
  bibu schema
  bibu schema pr comment add
  bibu schema --full pr
```

```text
bibu schema [OPTIONS] [PATH]...
```

| Argument | Description | Notes |
|---|---|---|
| `--full` | Include the output schema of every command, not only of a single named one | default `false` |
| `<PATH>` | Command path to describe, e.g. `pr comment add` | repeatable |

**Output** (the command tree: commands, arguments, output schemas, exit codes)

| Field | Type | Description |
|---|---|---|
| `schema_version` | integer | Version of this document's layout; bumped on incompatible changes. |
| `name` | string | Always `bibu`. |
| `version` | string | Version of the bibu binary. |
| `command` | object | A command: name, path, aliases, about, usage, args, subcommands and output. |
| `exit_codes` | array of object | The exit-code contract: code, error_code and meaning. |
| `error_output` | object | JSON Schema of the error envelope printed on stderr. |

## `bibu repo`

Show which repository bibu resolved (from --repo, BIBU_REPO or the git remote)

```text
bibu repo [OPTIONS]
```

**Output** (the resolved workspace and repository slug)

| Field | Type | Description |
|---|---|---|
| `workspace` | string | Workspace slug. |
| `slug` | string | Repository slug. |
