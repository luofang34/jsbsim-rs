#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Fails when the quality-gates steps of .github/workflows/ci.yml are not
# exactly the gates of scripts/ci.sh, one `scripts/ci.sh <gate>` each in the
# script's order, or when a workflow or action under .github uses another
# repository's action by anything but a full commit SHA, which a tag move
# could otherwise swap underneath CI, or when one reads a stored registry
# token, which crates.io trusted publishing replaces. Usage:
#   scripts/check-ci-workflow.sh
# JSBSIM_CI_WORKFLOW names the workflow to check in place of ci.yml and
# JSBSIM_CI_GITHUB_DIR the directory searched for uses: lines in place of
# .github (for scripts/check-ci-workflow.test.sh).
set -euo pipefail
cd "$(dirname "$0")/.."

workflow="${JSBSIM_CI_WORKFLOW:-.github/workflows/ci.yml}"
github_dir="${JSBSIM_CI_GITHUB_DIR:-.github}"
failures=0
fail() {
  echo "check-ci-workflow: $*" >&2
  failures=$((failures + 1))
}

expected=$(scripts/ci.sh --list)
found=$(sed -n 's/^ *run: scripts\/ci\.sh \([a-z-]*\) *$/\1/p' "$workflow")
if [ "$expected" != "$found" ]; then
  fail "$workflow runs these scripts/ci.sh gates:"
  diff <(printf '%s\n' "$expected") <(printf '%s\n' "$found") >&2 || true
  fail "(< in scripts/ci.sh --list only, > in $workflow only, or out of order)"
fi

while IFS= read -r line; do
  fail "not pinned to a commit SHA: $line"
done < <(grep -rnE '^\s*-?\s*uses:\s*' "$github_dir" \
  | grep -vE 'uses:\s*\./' \
  | grep -vE 'uses:\s*[^@[:space:]]+@[0-9a-f]{40}(\s|$)' || true)

while IFS= read -r line; do
  fail "reads a stored registry token: $line"
done < <(grep -rn 'secrets\.CARGO_REGISTRY_TOKEN' "$github_dir" || true)

if [ "$failures" -gt 0 ]; then
  echo "check-ci-workflow: $failures problem(s)" >&2
  exit 1
fi
echo "check-ci-workflow: ok"
