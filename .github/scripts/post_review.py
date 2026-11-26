"""Turn the output of `claude -p --output-format json` into a PR review and a commit status.

Usage:
  post_review.py verdict <claude-output.json>
      Prints the commit status state (success or failure) and its description, one per line.
      Exits non-zero when the output cannot be read, so an unreadable review never passes.
  post_review.py post <claude-output.json> <owner/repo> <pr-number> <head-sha>
      Posts the findings as one PR review. Needs the `gh` CLI with GH_TOKEN set.
"""

import json
import re
import subprocess
import sys

HUNK = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@")
LABELS = {"blocker": "\U0001F534 **Blocker**", "should_fix": "\U0001F7E0 **Should fix**"}


class ReviewError(Exception):
    pass


def extract(data):
    if data.get("is_error"):
        raise ReviewError(f"claude reported an error: {data.get('result')}")
    out = data.get("structured_output")
    if not isinstance(out, dict):
        raise ReviewError("claude output has no structured_output")
    return out


def blocker_count(out):
    return sum(f["severity"] == "blocker" for f in out.get("findings", []))


def blockers_label(count):
    if count == 0:
        return "No blockers"
    return f"{count} blocker{'s' if count > 1 else ''}"


def verdict(data):
    """The commit status for a Claude response: (state, description). Raises on a bad response,
    so a review that could not be read never passes."""
    count = blocker_count(extract(data))
    if count == 0:
        return "success", blockers_label(0)
    return "failure", f"{blockers_label(count)}, see the review"


def valid_lines(patch):
    """New-side line numbers of a unified diff patch; only these accept inline comments."""
    lines, current = set(), None
    for raw in (patch or "").splitlines():
        m = HUNK.match(raw)
        if m:
            current = int(m.group(1))
        elif current is None or raw.startswith(("-", "\\")):
            continue
        else:
            lines.add(current)
            current += 1
    return lines


def label(severity):
    return LABELS.get(severity, severity)


def build_review(out, patches, sha):
    """`patches` maps each changed file to its patch text (None when GitHub gives none)."""
    inline, loose = [], []
    for f in out.get("findings", []):
        path, line = f["file"], f["line"]
        if line in valid_lines(patches.get(path)):
            body = f"{label(f['severity'])}\n\n{f['message']}"
            inline.append({"path": path, "line": line, "side": "RIGHT", "body": body})
        else:
            loose.append(f"- {label(f['severity'])} `{path}:{line}`: {f['message']}")

    parts = [out.get("summary", "").strip()]
    if not out.get("findings"):
        parts.append("No issues found.")
    if loose:
        parts.append("Findings outside the changed lines:\n\n" + "\n".join(loose))
    parts.append(f"**{blockers_label(blocker_count(out))}**")
    return {
        "commit_id": sha,
        "event": "COMMENT",
        "body": "\n\n".join(p for p in parts if p),
        "comments": inline,
    }


def gh(*args, stdin=None):
    done = subprocess.run(
        ["gh", *args], input=stdin, capture_output=True, text=True, check=False
    )
    if done.returncode != 0:
        raise ReviewError(f"gh {' '.join(args)} failed: {done.stderr.strip()}")
    return done.stdout


def changed_patches(repo, pr):
    out = gh("api", f"repos/{repo}/pulls/{pr}/files", "--paginate", "--jq", ".[]")
    files = (json.loads(line) for line in out.splitlines() if line.strip())
    return {f["filename"]: f.get("patch") for f in files}


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def main(argv):
    command = argv[1] if len(argv) > 1 else ""
    if command == "verdict" and len(argv) == 3:
        state, description = verdict(load(argv[2]))
        print(state)
        print(description)
    elif command == "post" and len(argv) == 6:
        path, repo, pr, sha = argv[2:6]
        review = build_review(extract(load(path)), changed_patches(repo, pr), sha)
        gh("api", f"repos/{repo}/pulls/{pr}/reviews", "--input", "-", stdin=json.dumps(review))
        print(f"posted review with {len(review['comments'])} inline comment(s)")
    else:
        raise ReviewError(
            "usage: post_review.py verdict <output.json> | "
            "post <output.json> <owner/repo> <pr> <head-sha>"
        )


if __name__ == "__main__":
    try:
        main(sys.argv)
    except ReviewError as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
