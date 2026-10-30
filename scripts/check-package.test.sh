#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-package.sh rejects a package listing that ships data
# or tools, or that lacks a licence text, and accepts the real listing.
set -euo pipefail
cd "$(dirname "$0")/.."

checker=scripts/check-package.sh
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

cargo package --list -p jsbsim --offline --allow-dirty 2> /dev/null > "$scratch/real"

expect() { # expect <pass|fail> <name> <listing file>
  local got
  if JSBSIM_PACKAGE_LIST="$3" "$checker" > "$scratch/$2.log" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$1" ]; then
    echo "check-package.test: $2: expected $1, got $got" >&2
    cat "$scratch/$2.log" >&2
    exit 1
  fi
}

expect pass real "$scratch/real"
for planted in oracle/goldens/ball/reference.csv oracle/nasa/manifest.toml \
  tools/oracle/src/main.rs public-api.txt src/data.bin; do
  name=$(echo "$planted" | tr '/.' '__')
  { cat "$scratch/real"; echo "$planted"; } > "$scratch/$name"
  expect fail "$name" "$scratch/$name"
done
for missing in LICENSE LICENSES/GeographicLib-MIT.txt LICENSES/optimized-routines.txt README.md; do
  name=missing_$(echo "$missing" | tr '/.' '__')
  grep -vxF -e "$missing" "$scratch/real" > "$scratch/$name"
  expect fail "$name" "$scratch/$name"
done
echo "check-package.test: ok"
