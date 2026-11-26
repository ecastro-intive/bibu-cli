#!/bin/bash
# Fetch the findings state of the last review of this PR into prev/review-state.json.
# Needs REPO, PR and GH_TOKEN. Never fails the job: no usable state just means a review from
# scratch, which can only report more findings, never fewer.
name="review-state-pr-$PR"
trusted=$(printf '.github/workflows/claude-review.yml\tworkflow_run')

run_id=$(gh api "repos/$REPO/actions/artifacts?name=$name&per_page=1" \
  --jq '.artifacts[] | select(.expired == false) | .workflow_run.id') || run_id=
if [ -z "$run_id" ]; then
  echo "No previous review state for PR $PR"
  exit 0
fi

# Only trust state written by this workflow on main. A PR's own workflows can upload artifacts
# with any name, but they never run as a workflow_run.
origin=$(gh api "repos/$REPO/actions/runs/$run_id" --jq '[.path, .event] | @tsv') || origin=
if [ "$origin" != "$trusted" ]; then
  echo "::warning::Ignoring review state from an unexpected run (${origin:-unknown})"
  exit 0
fi

if ! gh run download "$run_id" --repo "$REPO" --name "$name" --dir prev; then
  echo "::warning::Could not download the previous review state, reviewing from scratch"
  exit 0
fi

# Depending on the gh version the file lands in prev/ or in a folder named after the artifact.
if [ ! -f prev/review-state.json ]; then
  found=$(find prev -name review-state.json 2>/dev/null | head -n 1 || true)
  if [ -n "$found" ]; then mv "$found" prev/review-state.json; fi
fi
