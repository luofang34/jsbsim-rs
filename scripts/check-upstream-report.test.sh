#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/upstream-report.sh reports only the upstream commits that
# touch what the port follows, records the newest of them on its first
# line, and prints nothing when no commit does. Each case feeds it stored
# API responses, so no network is used.
set -euo pipefail
cd "$(dirname "$0")/.."

reporter=scripts/upstream-report.sh
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
pin=$(sed -n 's/^upstream_commit = "\([0-9a-f]\{40\}\)"$/\1/p' oracle/goldens/ball/manifest.toml)
repo=repos_JSBSim-Team_jsbsim
ported=1111111111111111111111111111111111111111
skipped=2222222222222222222222222222222222222222

fail() {
  echo "check-upstream-report.test: $*" >&2
  exit 1
}

# commit_json <filename>...: a commit response touching those files.
commit_json() {
  local files="" f
  for f in "$@"; do
    files+="${files:+,}{\"filename\":\"$f\",\"additions\":3,\"deletions\":1}"
  done
  printf '{"files":[%s]}\n' "$files"
}

# fixtures <dir> <total_commits> <sha:date:title>...: a compare response
# listing those commits, and tags with the pin as v1.0.0 under v1.0.1.
fixtures() {
  local dir="$1" total="$2" commits="" entry
  shift 2
  mkdir -p "$dir"
  for entry in "$@"; do
    IFS=: read -r sha date title <<<"$entry"
    commits+="${commits:+,}{\"sha\":\"$sha\",\"commit\":{\"committer\":{\"date\":\"${date}T00:00:00Z\"},\"message\":\"$title\\n\\nbody\"}}"
  done
  printf '{"total_commits":%s,"commits":[%s]}\n' "$total" "$commits" > "$dir/${repo}_compare_$pin...master.json"
  printf '[{"name":"v1.0.1","commit":{"sha":"%s"}},{"name":"v1.0.0","commit":{"sha":"%s"}}]\n' "$skipped" "$pin" > "$dir/${repo}_tags.json"
}

report() {
  JSBSIM_UPSTREAM_FIXTURES="$1" "$reporter"
}

dir="$scratch/mixed"
fixtures "$dir" 3 "$ported:2026-01-02:Port me" "$skipped:2026-01-03:Skip me"
commit_json src/models/FGAircraft.cpp aircraft/c172x/c172x.xml README.md > "$dir/${repo}_commits_$ported.json"
commit_json src/simgear/xml/expat.h JSBSim.vcxproj src/JSBSim.vcxproj.filters python/setup.py > "$dir/${repo}_commits_$skipped.json"
out=$(report "$dir")
[ "$(sed -n 1p <<<"$out")" = "<!-- upstream-sync last-relevant: $ported -->" ] ||
  fail "first line does not record the ported commit: $(sed -n 1p <<<"$out")"
for want in "Port me" '`src/models/FGAircraft.cpp` +3 -1' '`aircraft/c172x/c172x.xml`' \
    "has 3 commit(s)" "(v1.0.0)" "1 of them" "newest upstream tag is v1.0.1" "oldest 2 of the 3"; do
  grep -qF -- "$want" <<<"$out" || fail "report lacks '$want':"$'\n'"$out"
done
for unwanted in "Skip me" README.md expat.h vcxproj python/; do
  if grep -qF -- "$unwanted" <<<"$out"; then fail "report lists '$unwanted':"$'\n'"$out"; fi
done

dir="$scratch/quiet"
fixtures "$dir" 1 "$skipped:2026-01-03:Skip me"
commit_json src/simgear/xml/expat.h .github/workflows/x.yml > "$dir/${repo}_commits_$skipped.json"
out=$(report "$dir")
[ -z "$out" ] || fail "nothing relevant, yet it printed:"$'\n'"$out"

echo "check-upstream-report.test: ok"
