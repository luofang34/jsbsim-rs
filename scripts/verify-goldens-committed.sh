#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Run after committing: fails unless every file under oracle/goldens and
# oracle/inputs on disk is in the commit (default HEAD). ci.sh and
# oracle-ci.sh read the working tree, so they cannot see a commit that
# left golden files out, as one made through a stale git index can.
set -euo pipefail
cd "$(dirname "$0")/.."

commit="${1:-HEAD}"
status=0
for dir in oracle/goldens oracle/inputs oracle/derived oracle/nasa; do
  on_disk=$(find "$dir" -type f | LC_ALL=C sort)
  committed=$(git ls-tree -r --name-only "$commit" -- "$dir" | LC_ALL=C sort)
  missing=$(LC_ALL=C comm -23 <(echo "$on_disk") <(echo "$committed"))
  if [ -n "$missing" ]; then
    echo "verify-goldens-committed: not in $commit:" >&2
    echo "$missing" | sed 's/^/  /' >&2
    status=1
  fi
done
[ "$status" -eq 0 ] && echo "verify-goldens-committed: ok"
exit "$status"
