#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves the checks gate of scripts/ci.sh runs each scripts/check-*.sh and
# each scripts/*.test.sh once, runs no other script, and fails when one of
# them does. The real gate runs this test, so a copy of ci.sh runs here
# beside planted scripts.
set -euo pipefail
cd "$(dirname "$0")/.."

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir "$scratch/scripts"
command cp -f scripts/ci.sh "$scratch/scripts/ci.sh"

fail() {
  echo "ci.test: $*" >&2
  exit 1
}

# plant <name> [exit status]: a script that records its run.
plant() {
  cat > "$scratch/scripts/$1" <<EOF
#!/usr/bin/env bash
basename "\$0" >> "$scratch/ran"
exit ${2:-0}
EOF
  chmod +x "$scratch/scripts/$1"
}

gate() {
  rm -f "$scratch/ran"
  "$scratch/scripts/ci.sh" checks > "$scratch/gate.log" 2>&1
}

for name in check-a.sh check-a.test.sh check-b.sh other.test.sh other.sh; do
  plant "$name"
done
gate || fail "every script passed, yet the gate failed: $(cat "$scratch/gate.log")"
ran=$(LC_ALL=C sort "$scratch/ran")
want=$'check-a.sh\ncheck-a.test.sh\ncheck-b.sh\nother.test.sh'
[ "$ran" = "$want" ] || fail "the gate ran:"$'\n'"$ran"

plant check-b.sh 1
if gate; then fail "a failing check passed the gate"; fi
if grep -qF other.test.sh "$scratch/ran"; then fail "the gate ran on past a failing check"; fi

plant check-b.sh
plant other.test.sh 1
if gate; then fail "a failing test passed the gate"; fi

echo "ci.test: ok"
