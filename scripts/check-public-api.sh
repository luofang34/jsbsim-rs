#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Fails when the public API of the jsbsim crate differs from the committed
# listing, so every API change shows up in review as a change to that file.
# Items hidden with #[doc(hidden)], among them jsbsim::oracle_support, are
# not part of the listing. Usage:
#   scripts/check-public-api.sh            # compare
#   scripts/check-public-api.sh --bless    # rewrite the listing
# Needs cargo-public-api (`cargo install --locked cargo-public-api@0.52.0`)
# and nothing from the network: it reads rustdoc JSON that the toolchain of
# rust-toolchain.toml writes. JSBSIM_PUBLIC_API_LISTING overrides the
# listing compared against (for scripts/check-public-api.test.sh).
set -euo pipefail
cd "$(dirname "$0")/.."

listing="${JSBSIM_PUBLIC_API_LISTING:-crates/jsbsim/public-api.txt}"
tool_version=0.52.0
target_dir=target/public-api

if ! command -v cargo-public-api > /dev/null; then
  echo "check-public-api: cargo-public-api is not installed; run" >&2
  echo "  cargo install --locked cargo-public-api@$tool_version" >&2
  exit 1
fi
found=$(cargo public-api --version | awk '{print $2}')
if [ "$found" != "$tool_version" ]; then
  echo "check-public-api: needs cargo-public-api $tool_version, found $found; run" >&2
  echo "  cargo install --locked cargo-public-api@$tool_version" >&2
  exit 1
fi

# Rustdoc JSON is unstable. A nightly would render std paths its own way
# (alloc::rcs::arc::Arc), so the pinned stable toolchain writes it instead,
# with RUSTC_BOOTSTRAP unlocking the flag. Its own target directory keeps
# that build from invalidating the normal one.
rm -f "$target_dir/doc/jsbsim.json"
RUSTC_BOOTSTRAP=1 RUSTDOCFLAGS="-Z unstable-options --output-format json" \
  cargo doc -q -p jsbsim --features fs --no-deps --target-dir "$target_dir"
actual="$target_dir/public-api.txt"
cargo public-api --rustdoc-json "$target_dir/doc/jsbsim.json" --omit blanket-impls \
  --color never > "$actual"

if [ "${1:-}" = "--bless" ]; then
  cp -f "$actual" "$listing"
  echo "check-public-api: wrote $listing"
  exit 0
fi
if ! diff -u "$listing" "$actual"; then
  echo "check-public-api: the public API differs from $listing (diff above)." >&2
  echo "If the change is intended, run scripts/check-public-api.sh --bless and commit $listing." >&2
  exit 1
fi
echo "check-public-api: ok"
