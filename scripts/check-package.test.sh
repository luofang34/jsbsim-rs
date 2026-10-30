#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-package.sh rejects a package listing that ships data
# or tools, or that lacks a licence text, and a manifest that pins or caps
# a dependency or adds a public feature, and accepts the real listing.
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
expect_manifest() { # expect_manifest <pass|fail> <name> <manifest>
  local got
  if JSBSIM_PACKAGE_LIST="$scratch/real" JSBSIM_PACKAGE_MANIFEST="$3" "$checker" > "$scratch/$2.log" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$1" ]; then
    echo "check-package.test: $2: expected $1, got $got" >&2
    cat "$scratch/$2.log" >&2
    exit 1
  fi
}
printf '[package]\nname = "jsbsim"\ndescription = "x < y"\n\n[dependencies.sha2]\nversion = "0.11"\n' > "$scratch/caret.toml"
expect_manifest pass caret "$scratch/caret.toml"
for req in '=0.11.1' '<0.12' '>=0.10, <0.12'; do
  name=pinned_$(echo "$req" | tr -c 'a-z0-9' '_')
  sed "s/^version = \"0.11\"$/version = \"$req\"/" "$scratch/caret.toml" > "$scratch/$name.toml"
  expect_manifest fail "$name" "$scratch/$name.toml"
done
printf '[package]\nname = "jsbsim"\n\n[target.wasm32-unknown-unknown.dependencies]\nsha2 = "=0.11.1"\n' > "$scratch/inline.toml"
expect_manifest fail inline "$scratch/inline.toml"
printf '[package]\nname = "jsbsim"\n\n[features]\nfs = []\n__oracle = []\n' > "$scratch/features.toml"
expect_manifest pass features "$scratch/features.toml"
printf '[package]\nname = "jsbsim"\n\n[features]\nfs = []\nplatform-libm = []\n' > "$scratch/public_feature.toml"
expect_manifest fail public_feature "$scratch/public_feature.toml"
echo "check-package.test: ok"
