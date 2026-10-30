#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Fails when the jsbsim crate package would ship anything beyond its Rust
# sources, README and licence texts (oracle data, NASA data, goldens,
# tools, the API listing), or when it would lack a licence text. Builds the
# .crate offline and checks the archive itself, so the symlinked LICENSE
# and LICENSES are seen as the regular files cargo packs. Usage:
#   scripts/check-package.sh
# JSBSIM_PACKAGE_LIST names a file to check in place of the listing (for
# scripts/check-package.test.sh); the archive check is then skipped.
set -euo pipefail
cd "$(dirname "$0")/.."

failures=0
fail() {
  echo "check-package: $*" >&2
  failures=$((failures + 1))
}

if [ -n "${JSBSIM_PACKAGE_LIST:-}" ]; then
  list=$(cat "$JSBSIM_PACKAGE_LIST")
else
  list=$(cargo package --list -p jsbsim --offline --allow-dirty)
fi

# Everything listed must be one of these; cargo adds the first four.
allowed='^(Cargo\.toml|Cargo\.toml\.orig|Cargo\.lock|\.cargo_vcs_info\.json|README\.md|LICENSE|LICENSES/[A-Za-z0-9._-]+\.txt|src/[A-Za-z0-9_/]+\.rs)$'
while IFS= read -r path; do
  fail "packages $path, which is not a source, README or licence text"
done < <(printf '%s\n' "$list" | grep -vE "$allowed" || true)

notices=(LICENSE LICENSES/*.txt)
required=(README.md src/lib.rs "${notices[@]}")
for path in "${required[@]}"; do
  printf '%s\n' "$list" | grep -qxF -e "$path" || fail "does not package $path"
done

# The archive must hold the licence texts themselves, not links to them.
if [ -n "${JSBSIM_PACKAGE_LIST:-}" ]; then
  notices=()
else
  cargo package -q -p jsbsim --offline --allow-dirty --no-verify
  crate=target/package/jsbsim-$(sed -n 's/^version = "\(.*\)"$/\1/p' crates/jsbsim/Cargo.toml).crate
  entries=$(tar -tvzf "$crate")
fi
for path in ${notices[@]+"${notices[@]}"}; do
  if ! printf '%s\n' "$entries" | grep -E "^-.* jsbsim-[^/]+/$path\$" > /dev/null; then
    fail "$crate does not hold $path as a regular file"
  fi
done

if [ "$failures" -gt 0 ]; then
  echo "check-package: $failures problem(s)" >&2
  exit 1
fi
echo "check-package: ok ($(printf '%s\n' "$list" | wc -l | tr -d ' ') files)"
