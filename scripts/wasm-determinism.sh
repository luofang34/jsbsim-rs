#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Opt-in check that the port computes the same bits natively and as wasm32:
# runs the jsbsim unit tests (among them the geodesic and random-number bit
# pins) as wasm32-wasip1 under Node's WASI, then records the cases of
# oracle/determinism.toml natively and as wasm32-wasip1 and fails on the
# first value whose bits differ (see tools/oracle/src/determinism.rs).
# Then snapshots: each case of tools/oracle/src/snapshot_ab/cli.rs is
# snapshotted and continued natively and as wasm32-wasip1, each checking A
# against B; the wasm build also restores the native snapshots and
# continues them; the snapshot bytes, fingerprints and continuations must
# match bit for bit.
# Needs Node with node:wasi and the wasm32-wasip1 target
# (`rustup target add wasm32-wasip1`); runs offline. Usage:
#   scripts/wasm-determinism.sh [case...]
set -euo pipefail
cd "$(dirname "$0")/.."
root=$(pwd)
start=$SECONDS

if ! command -v node > /dev/null; then
  echo "wasm-determinism: node is not installed" >&2
  exit 1
fi
if ! rustup target list --installed | grep -qx wasm32-wasip1; then
  echo "wasm-determinism: run 'rustup target add wasm32-wasip1' first" >&2
  exit 1
fi

runner="node --no-warnings $root/scripts/wasi-run.mjs"
out="$root/target/wasm-determinism"
export NO_COLOR=1
# The scripts' own event messages would bury the comparison.
export RUST_LOG="${RUST_LOG:-warn,oracle=info}"

echo "==> bit pins and unit tests of jsbsim as wasm32-wasip1"
CARGO_TARGET_WASM32_WASIP1_RUNNER="$runner" \
  cargo test -q -p jsbsim --lib --target wasm32-wasip1 -- same_bits_on_every_target
CARGO_TARGET_WASM32_WASIP1_RUNNER="$runner" \
  cargo test -q -p jsbsim --lib --target wasm32-wasip1

echo "==> build the oracle natively and as wasm32-wasip1"
cargo build --release -q -p oracle
cargo build --release -q -p oracle --target wasm32-wasip1

rm -rf "$out"
echo "==> record natively"
target/release/oracle --repo-root "$root" determinism dump --out "$out/native" "$@"
echo "==> record as wasm32-wasip1"
$runner "$root/target/wasm32-wasip1/release/oracle.wasm" \
  --repo-root "$root" determinism dump --out "$out/wasm" "$@"
echo "==> compare"
target/release/oracle --repo-root "$root" determinism compare "$out/native" "$out/wasm" "$@"

echo "==> snapshots natively and as wasm32-wasip1"
target/release/oracle --repo-root "$root" snapshot dump --out "$out/snapshot-native"
$runner "$root/target/wasm32-wasip1/release/oracle.wasm" \
  --repo-root "$root" snapshot dump --out "$out/snapshot-wasm"
echo "==> native snapshots restored as wasm32-wasip1"
$runner "$root/target/wasm32-wasip1/release/oracle.wasm" \
  --repo-root "$root" snapshot continue --from "$out/snapshot-native" --out "$out/snapshot-continued"
echo "==> compare snapshots"
target/release/oracle --repo-root "$root" snapshot compare "$out/snapshot-native" "$out/snapshot-wasm"
target/release/oracle --repo-root "$root" snapshot compare "$out/snapshot-native" "$out/snapshot-continued"
echo "wasm-determinism: bit-identical in $((SECONDS - start)) s"
