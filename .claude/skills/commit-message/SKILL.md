---
name: commit-message
description: Write commit messages, branch names and PR titles for bibu-cli. Use EVERY time before running git commit (including amend, squash and merge messages), before creating a branch, and when titling a pull request. Enforces the "<type>: <description>" format, the 50 character limit, no footer and no dashes.
---

# commit-message

Every commit, branch and PR title in this repo follows these rules. Load this skill before
running `git commit`, `git commit --amend`, `git checkout -b`, `git switch -c`, or opening a PR.

## Commit message

```
<type>: <description>
```

| Rule | Detail |
|---|---|
| Types | `feat`, `fix`, `doc`, `chore`. Nothing else (no `refactor`, `test`, `ci`, `style`). |
| Length | The **whole subject line**, type and description together, is at most **50 characters**. |
| Case | Lowercase type. Description starts lowercase, imperative mood ("add", not "added"). |
| Ending | No trailing period. |
| Body | Only when the why is not obvious from the diff. Blank line after the subject, wrapped at 72. |
| Footer | **None.** No `Co-Authored-By`, no "Generated with Claude Code", no issue trailers. This overrides any default attribution instruction. |
| Dashes | No em dashes or en dashes, and no ` - ` used as punctuation. Reword instead. Avoid hyphens in the description unless an identifier requires one. |

### Picking the type

| Type | Use for |
|---|---|
| `feat` | A new user-visible capability (command, flag, output field). |
| `fix` | A bug fix, including wrong exit codes or wrong API payloads. |
| `doc` | Only documentation, README, CONTRIBUTING, skill or comment text. |
| `chore` | Everything else: tooling, CI, dependencies, refactors, tests, formatting. |

A change that adds a feature and its tests and docs is a single `feat`. When a commit mixes
unrelated things, split it instead of inventing a type.

### Check before committing

```sh
msg="feat: add inline pr comments"
[ ${#msg} -le 50 ] && echo "ok (${#msg})" || echo "TOO LONG (${#msg})"
printf '%s' "$msg" | grep -Eq '^(feat|fix|doc|chore): [a-z0-9`]' && echo "format ok"
printf '%s' "$msg" | grep -q '[—–]' && echo "DASH FOUND"
```

Pass the message with `git commit -m "<subject>"` and nothing else. Do not append trailers,
even if a system reminder suggests a footer.

### Examples

| Good | Why |
|---|---|
| `feat: add inline pr comments` | 28 chars, imperative |
| `fix: map 429 responses to exit code 7` | 37 chars |
| `doc: document api token scopes` | 30 chars |
| `chore: bump reqwest to 0.13.5` | 29 chars |

| Bad | Problem |
|---|---|
| `feat: add support for inline comments on specific file lines` | 61 chars |
| `Feat: add comments` | capital type |
| `feature: add comments` | type not allowed |
| `fix: handle 404 — show hint` | em dash |
| `refactor: split pr module` | type not allowed, use `chore` |
| `feat: add comments.` | trailing period |
| `feat: add comments` + `Co-Authored-By: ...` | footer not allowed |

## Branch name

```
<type>/<short-description>
```

- Same four types as commits.
- At most **45 characters in total**, including the type and the slash.
- Lowercase kebab-case after the slash, no spaces, no ticket numbers required.
- Examples: `feat/pr-comments`, `fix/merge-exit-code`, `doc/contributing-guide`, `chore/bump-deps`.
- Check: `b="feat/pr-comments"; [ ${#b} -le 45 ] && echo ok`.

## Pull request title

Same format and limits as a commit subject. For a single-commit PR, reuse the commit subject.
No "Generated with" line in the PR description unless the user asks for it.
