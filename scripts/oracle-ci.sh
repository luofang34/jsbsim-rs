#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Oracle CI against the C++ JSBSim in JSBSIM_ROOT (default ~/jsbsim):
# regenerates every golden case under target/ and fails if any departs from
# the committed reference (beyond oracle/tolerances.toml for series, at all
# for transcripts) or was made from another commit or with other compiler
# flags; then parses every XML file of the JSBSim tree in C++ and Rust and
# fails if the element trees differ; then loads every corpus aircraft in
# C++ and Rust (see tools/oracle/src/corpus/models.rs); then runs the Rust
# port on every golden case and compares it with the reference.
set -euo pipefail
cd "$(dirname "$0")/.."

export JSBSIM_ROOT="${JSBSIM_ROOT:-$HOME/jsbsim}"
export NO_COLOR=1
cargo run --release -q -p oracle -- check-drift "$@"
cargo run --release -q -p oracle -- check-xml
cargo run --release -q -p oracle -- check-models
cargo run --release -q -p oracle -- port
