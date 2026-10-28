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
gate cargo test --workspace --all-targets
RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" gate cargo doc --workspace --no-deps
gate cargo build --workspace --release
gate cargo build -p jsbsim --target wasm32-unknown-unknown
for check in scripts/check-*.sh; do
  gate "$check"
done
echo "==> all gates passed"
