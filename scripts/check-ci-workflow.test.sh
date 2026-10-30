#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-ci-workflow.sh accepts the real workflow and rejects
# one that drops, adds or reorders a gate, or uses an action by tag.
set -euo pipefail
cd "$(dirname "$0")/.."

checker=scripts/check-ci-workflow.sh
real=.github/workflows/ci.yml
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

expect() { # expect <pass|fail> <name> <workflow> [github dir]
  local got
  if JSBSIM_CI_WORKFLOW="$3" JSBSIM_CI_GITHUB_DIR="${4:-.github}" "$checker" \
    > "$scratch/$2.log" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$1" ]; then
    echo "check-ci-workflow.test: $2: expected $1, got $got" >&2
    cat "$scratch/$2.log" >&2
    exit 1
  fi
}

expect pass real "$real"

grep -v 'run: scripts/ci.sh doctest$' "$real" > "$scratch/dropped.yml"
expect fail dropped "$scratch/dropped.yml"

awk '{ print } /run: scripts\/ci\.sh wasm$/ { print "        run: scripts/ci.sh bench" }' "$real" > "$scratch/added.yml"
expect fail added "$scratch/added.yml"

sed -e 's|run: scripts/ci.sh fmt$|run: scripts/ci.sh @@|' \
  -e 's|run: scripts/ci.sh clippy$|run: scripts/ci.sh fmt|' \
  -e 's|run: scripts/ci.sh @@$|run: scripts/ci.sh clippy|' "$real" > "$scratch/reordered.yml"
expect fail reordered "$scratch/reordered.yml"

mkdir -p "$scratch/github/workflows"
sed 's|uses: actions/checkout@[0-9a-f]\{40\}|uses: actions/checkout@v7|' "$real" \
  > "$scratch/github/workflows/ci.yml"
expect fail tag "$real" "$scratch/github"
command cp -f "$real" "$scratch/github/workflows/ci.yml"
expect pass pinned "$real" "$scratch/github"

echo "check-ci-workflow.test: ok"
