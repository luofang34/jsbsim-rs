#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/verify-goldens-committed.sh fails for a commit that left
# out one file of each directory it guards. Each case builds a commit
# object from the files on disk through a private index, so neither the
# real index nor any ref changes.
set -euo pipefail
cd "$(dirname "$0")/.."

checker=scripts/verify-goldens-committed.sh
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
dirs=(oracle/goldens oracle/inputs oracle/derived oracle/scenarios oracle/nasa oracle/snapshot)

# A commit holding every guarded file on disk, minus "$1" when given.
commit_without() {
  local index="$scratch/index"
  rm -f "$index"
  GIT_INDEX_FILE="$index" git read-tree --empty
  GIT_INDEX_FILE="$index" git add -f -- "${dirs[@]}"
  if [ -n "${1:-}" ]; then
    GIT_INDEX_FILE="$index" git rm -q --cached -- "$1"
  fi
  local tree
  tree=$(GIT_INDEX_FILE="$index" git write-tree)
  git -c user.name=self-test -c user.email=self-test@localhost \
    commit-tree -m "verify-goldens-committed self-test" "$tree"
}

if ! "$checker" "$(commit_without)" > "$scratch/full.log" 2>&1; then
  echo "check-verify-goldens-committed.test: a complete commit failed" >&2
  cat "$scratch/full.log" >&2
  exit 1
fi
for dir in "${dirs[@]}"; do
  file=$(find "$dir" -type f | LC_ALL=C sort | sed -n 1p)
  if "$checker" "$(commit_without "$file")" > "$scratch/case.log" 2>&1; then
    echo "check-verify-goldens-committed.test: missing $file passed" >&2
    exit 1
  fi
  if ! grep -qxF "  $file" "$scratch/case.log"; then
    echo "check-verify-goldens-committed.test: $file not named" >&2
    cat "$scratch/case.log" >&2
    exit 1
  fi
done
echo "check-verify-goldens-committed.test: ok"
