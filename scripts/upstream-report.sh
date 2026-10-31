#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Prints a Markdown report of the upstream JSBSim master commits since the
# port's pin (upstream_commit in oracle/goldens/ball/manifest.toml) that
# touch C++ sources or aircraft, engine, systems or script data, and prints
# nothing when none do. The report's first line records the newest such
# commit, so .github/workflows/upstream-sync.yml reports each change once.
# It only reads the public GitHub API. Usage:
#   scripts/upstream-report.sh
# JSBSIM_UPSTREAM_FIXTURES names a directory of stored API responses read in
# place of the API (for scripts/check-upstream-report.test.sh): the response
# for path a/b/c is a_b_c.json.
set -euo pipefail
cd "$(dirname "$0")/.."

repo=JSBSim-Team/jsbsim
manifest=oracle/goldens/ball/manifest.toml

api() {
  if [ -n "${JSBSIM_UPSTREAM_FIXTURES:-}" ]; then
    cat "$JSBSIM_UPSTREAM_FIXTURES/$(printf '%s' "$1" | tr '/' '_').json"
  else
    gh api "$1"
  fi
}

# What the port follows: everything under src/ but the bundled expat and
# the MSVC project files, and the data directories the goldens load.
relevant() {
  grep -E '^(src|aircraft|engine|systems|scripts)/' |
    grep -vE '^src/simgear/xml/|\.vcxproj(\.filters)?$' || true
}

pin=$(sed -n 's/^upstream_commit = "\([0-9a-f]\{40\}\)"$/\1/p' "$manifest")
if [ -z "$pin" ]; then
  echo "upstream-report: no upstream_commit in $manifest" >&2
  exit 1
fi

compare=$(api "repos/$repo/compare/$pin...master")
total=$(jq -r '.total_commits' <<<"$compare")
listed=$(jq -r '.commits | length' <<<"$compare")

body=""
newest=""
count=0
while IFS=$'\t' read -r sha date title; do
  files=$(api "repos/$repo/commits/$sha" |
    jq -r '.files[] | "\(.filename)\t+\(.additions) -\(.deletions)"' |
    awk -F'\t' '{ print $1 "\t" $2 }')
  kept=$(cut -f1 <<<"$files" | relevant)
  [ -n "$kept" ] || continue
  newest=$sha
  count=$((count + 1))
  body+="- [\`${sha:0:9}\`](https://github.com/$repo/commit/$sha) $date $title"$'\n'
  while IFS= read -r file; do
    size=$(awk -F'\t' -v f="$file" '$1 == f { print $2 }' <<<"$files")
    body+="  - \`$file\` $size"$'\n'
  done <<<"$kept"
done < <(jq -r '.commits[] | [.sha, .commit.committer.date[:10], (.commit.message | split("\n")[0])] | @tsv' <<<"$compare")

[ "$count" -gt 0 ] || exit 0

tags=$(api "repos/$repo/tags")
pin_tag=$(jq -r --arg pin "$pin" '[.[] | select(.commit.sha == $pin) | .name][0] // empty' <<<"$tags")
newest_tag=$(jq -r '.[0].name // empty' <<<"$tags")

echo "<!-- upstream-sync last-relevant: $newest -->"
echo "Upstream JSBSim master has $total commit(s) that the port's pin \`${pin:0:9}\`${pin_tag:+ ($pin_tag)} lacks; $count of them touch sources or data the port follows:"
echo
printf '%s' "$body"
if [ "$listed" -lt "$total" ]; then
  echo
  echo "Only the oldest $listed of the $total commits were listed by the compare API."
fi
if [ -n "$newest_tag" ] && [ "$newest_tag" != "$pin_tag" ]; then
  echo
  echo "The newest upstream tag is $newest_tag."
fi
echo
echo "Review each change against the port. Following them means moving upstream_commit in the oracle manifests and regenerating the goldens; closing this issue skips them until a newer change arrives."
