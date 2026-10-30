#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Local CI gates, in order. Stops at the first failing gate.
set -euo pipefail
cd "$(dirname "$0")/.."

gate() {
  echo "==> $*"
  "$@"
}

gate cargo fmt --all --check
gate cargo clippy --workspace --all-targets -- -D warnings
# The workspace build turns on jsbsim's __oracle feature for tools/oracle;
# this is jsbsim as a dependent builds it.
gate cargo clippy -p jsbsim --all-targets --features fs -- -D warnings
gate cargo test --workspace --all-targets
# --all-targets leaves out doc tests.
gate cargo test --workspace --doc
docflags="-D missing_docs -D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links"
RUSTDOCFLAGS="$docflags" gate cargo doc --workspace --no-deps
# As docs.rs builds it: without __oracle, so no link may lean on internals.
RUSTDOCFLAGS="$docflags" gate cargo doc -p jsbsim --features fs --no-deps --target-dir target/doc-default
gate cargo build --workspace --release
gate cargo build -p jsbsim --target wasm32-unknown-unknown
for check in scripts/check-*.sh; do
  gate "$check"
done
echo "==> all gates passed"
