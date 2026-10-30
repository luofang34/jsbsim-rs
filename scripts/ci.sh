#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# The CI gates, in order; .github/workflows/ci.yml runs each as a step of
# its quality-gates job, and scripts/check-ci-workflow.sh keeps the two
# lists the same. Stops at the first failing gate. Usage:
#   scripts/ci.sh            # every gate
#   scripts/ci.sh <gate>...  # the named gates
#   scripts/ci.sh --list     # the gate names, in order
set -euo pipefail
cd "$(dirname "$0")/.."

gates=(fmt clippy clippy-dependent test doctest doc doc-dependent release wasm checks)
docflags="-D missing_docs -D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links"

run() {
  echo "==> $*"
  "$@"
}

gate() {
  case "$1" in
    fmt) run cargo fmt --all --check ;;
    clippy) run cargo clippy --workspace --all-targets -- -D warnings ;;
    # The workspace build turns on jsbsim's __oracle feature for
    # tools/oracle; this is jsbsim as a dependent builds it. Every public
    # function that returns a Result documents its errors; the oracle
    # internals need not.
    clippy-dependent)
      run cargo clippy -p jsbsim --all-targets --features fs -- -D warnings -D clippy::missing_errors_doc
      ;;
    test) run cargo test --workspace --all-targets ;;
    # --all-targets leaves out doc tests.
    doctest) run cargo test --workspace --doc ;;
    doc) RUSTDOCFLAGS="$docflags" run cargo doc --workspace --no-deps ;;
    # As docs.rs builds it: without __oracle, so no link may lean on
    # internals.
    doc-dependent)
      RUSTDOCFLAGS="$docflags" run cargo doc -p jsbsim --features fs --no-deps --target-dir target/doc-default
      ;;
    release) run cargo build --workspace --release ;;
    wasm) run cargo build -p jsbsim --target wasm32-unknown-unknown ;;
    checks)
      for check in scripts/check-*.sh; do
        run "$check"
      done
      ;;
    *)
      echo "ci: no gate $1; gates: ${gates[*]}" >&2
      return 1
      ;;
  esac
}

if [ "${1:-}" = "--list" ]; then
  printf '%s\n' "${gates[@]}"
  exit 0
fi
if [ "$#" -eq 0 ]; then
  set -- "${gates[@]}"
fi
for name in "$@"; do
  gate "$name"
done
echo "==> gates passed: $*"
