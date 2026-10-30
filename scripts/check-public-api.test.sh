#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-public-api.sh fails, with a diff naming the item,
# when the committed listing misses an item of the public API, and fails on
# a #[doc(hidden)] item in the sources.
set -euo pipefail
cd "$(dirname "$0")/.."

checker=scripts/check-public-api.sh
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

item='pub fn jsbsim::Fdm::run(&mut self) -> core::result::Result<bool, jsbsim::error::Error>'
if ! grep -qxF -e "$item" crates/jsbsim/public-api.txt; then
  echo "check-public-api.test: the listing lacks $item" >&2
  exit 1
fi
grep -vxF -e "$item" crates/jsbsim/public-api.txt > "$scratch/listing.txt"
if JSBSIM_PUBLIC_API_LISTING="$scratch/listing.txt" "$checker" > "$scratch/log" 2>&1; then
  echo "check-public-api.test: a listing without Fdm::run passed" >&2
  exit 1
fi
if ! grep -qxF -e "+$item" "$scratch/log"; then
  echo "check-public-api.test: the failure does not show the missing item" >&2
  cat "$scratch/log" >&2
  exit 1
fi
mkdir "$scratch/src"
printf '#[doc(hidden)]\npub fn internal() {}\n' > "$scratch/src/lib.rs"
if JSBSIM_PUBLIC_API_SRC="$scratch/src" "$checker" > "$scratch/log" 2>&1; then
  echo "check-public-api.test: a #[doc(hidden)] item passed" >&2
  exit 1
fi
if ! grep -qF 'lib.rs:1:#[doc(hidden)]' "$scratch/log"; then
  echo "check-public-api.test: the failure does not name the #[doc(hidden)] item" >&2
  cat "$scratch/log" >&2
  exit 1
fi
echo "check-public-api.test: ok"
