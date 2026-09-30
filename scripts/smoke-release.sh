#!/usr/bin/env bash
# Release smoke test: proves that what a release would contain actually installs and upgrades.
#
#   1. builds the release artifacts (cargo-dist) for the committed HEAD  -> version X
#   2. builds them again with the patch version raised                   -> version X+1
#   3. installs X with the real installer, from a fake local "GitHub"
#   4. runs `bibu upgrade --check` and `bibu upgrade` and checks X+1 is installed
#
# Everything happens in a temporary git worktree and a throwaway HOME: your checkout, your
# ~/.local/bin and your Keychain are not touched. Needs `dist` (cargo install cargo-dist --locked),
# python3 and a committed HEAD. macOS / Linux only; Windows is covered by the CI build.
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
WORK=$(mktemp -d)
PORT=${SMOKE_PORT:-8766}
SERVER=""
cleanup() {
  [ -n "$SERVER" ] && { kill "$SERVER"; wait "$SERVER"; } 2>/dev/null || true
  git -C "$ROOT" worktree remove --force "$WORK/src" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

command -v dist >/dev/null || { echo "dist not found: cargo install cargo-dist --locked" >&2; exit 2; }
TARGET=$(rustc -vV | sed -n 's/^host: //p')
OLD=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
NEW=$(echo "$OLD" | awk -F. '{printf "%d.%d.%d", $1, $2, $3 + 1}')
echo "==> release smoke test: $OLD -> $NEW on $TARGET"

git -C "$ROOT" worktree add --detach "$WORK/src" HEAD >/dev/null 2>&1
build() { (cd "$WORK/src" && dist build --artifacts=all --target "$TARGET" >/dev/null 2>&1 && cp -R target/distrib "$1"); }

echo "==> building $OLD"
build "$WORK/rel-$OLD"
echo "==> building $NEW"
sed -i.bak "s/^version = \"$OLD\"/version = \"$NEW\"/" "$WORK/src/Cargo.toml" && rm -f "$WORK/src/Cargo.toml.bak"
build "$WORK/rel-$NEW"

serve() {  # serve VERSION
  [ -n "$SERVER" ] && { kill "$SERVER"; wait "$SERVER"; } 2>/dev/null || true
  python3 "$ROOT/scripts/fake_github.py" "$PORT" "$1" "$WORK/rel-$1" 2>"$WORK/server.log" &
  SERVER=$!
  sleep 1
}

export HOME="$WORK/home"; mkdir -p "$HOME"
export BIBU_INSTALLER_GHE_BASE_URL="http://127.0.0.1:$PORT/" BIBU_NO_MODIFY_PATH=1
BIBU="$HOME/.local/bin/bibu"

echo "==> installing $OLD with the real installer"
serve "$OLD"
sh "$WORK/rel-$OLD/bibu-installer.sh" >/dev/null 2>&1
[ "$("$BIBU" --version)" = "bibu $OLD" ] || { echo "FAIL: expected bibu $OLD after install" >&2; exit 1; }

echo "==> upgrading to $NEW"
serve "$NEW"
field() { python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }
CHECK=$("$BIBU" upgrade --check --json)
[ "$(echo "$CHECK" | field update_available)" = "True" ] || { echo "FAIL: --check did not see $NEW: $CHECK" >&2; exit 1; }
[ "$("$BIBU" --version)" = "bibu $OLD" ] || { echo "FAIL: --check must not install anything" >&2; exit 1; }
DONE=$("$BIBU" upgrade --json)
[ "$(echo "$DONE" | field upgraded)" = "True" ] || { echo "FAIL: upgrade did not run: $DONE" >&2; exit 1; }
[ "$("$BIBU" --version)" = "bibu $NEW" ] || { echo "FAIL: expected bibu $NEW after upgrade" >&2; exit 1; }
AGAIN=$("$BIBU" upgrade --json)
[ "$(echo "$AGAIN" | field update_available)" = "False" ] || { echo "FAIL: still offers an upgrade: $AGAIN" >&2; exit 1; }

echo "==> OK: installed $OLD, upgraded to $NEW, and the second upgrade was a no-op"
