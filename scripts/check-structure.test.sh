#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-structure.sh rejects each rule's violation: every case
# copies a passing tree, plants one violation and expects a failure.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
checker="$here/check-structure.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

spdx='// SPDX-License-Identifier: LGPL-2.1-or-later'

make_tree() {
  local dir="$1"
  mkdir -p "$dir/crates/a/src"
  printf '[package]\nname = "a"\n\n[lints]\nworkspace = true\n' > "$dir/crates/a/Cargo.toml"
  printf '%s\n//! Crate a.\n\npub mod b;\n' "$spdx" > "$dir/crates/a/src/lib.rs"
  printf '%s\n//! Module b.\n\nfn f() {\n    let x = 1;\n}\n' "$spdx" > "$dir/crates/a/src/b.rs"
}

expect() {
  local outcome="$1" name="$2" dir="$scratch/$2"
  shift 2
  make_tree "$dir"
  (cd "$dir" && eval "$*")
  if "$checker" "$dir" > "$dir.log" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$outcome" ]; then
    echo "check-structure.test: $name: expected $outcome, got $got" >&2
    cat "$dir.log" >&2
    exit 1
  fi
}

expect pass clean-tree true

# GeographicLib ports: LGPL-2.1-or-later AND MIT, its notice, and its origin.
notice="$here/../LICENSES/GeographicLib-MIT.txt"
mit_file() { # mit_file <spdx> <with-derived-line:yes|no> <with-notice:yes|no>
  echo "// SPDX-License-Identifier: $1"
  [ "$2" = yes ] && echo '// Derived from GeographicLib src/Geodesic.cpp: Copyright (c) Charles Karney.'
  [ "$3" = yes ] && sed 's#^#//   #' "$notice"
  echo '//! Module m.'
}
mit_case="mkdir -p LICENSES && cp \"$notice\" LICENSES/GeographicLib-MIT.txt"
expect pass mit-port "$mit_case && mit_file 'LGPL-2.1-or-later AND MIT' yes yes > crates/a/src/m.rs"
expect fail mit-notice-without-and "$mit_case && mit_file 'LGPL-2.1-or-later' yes yes > crates/a/src/m.rs"
expect fail mit-and-without-notice "$mit_case && mit_file 'LGPL-2.1-or-later AND MIT' yes no > crates/a/src/m.rs"
expect fail mit-and-without-origin "$mit_case && mit_file 'LGPL-2.1-or-later AND MIT' no yes > crates/a/src/m.rs"
expect fail mod-rs "printf '%s\n' \"$spdx\" > crates/a/src/mod.rs"
expect fail utils-rs "printf '%s\n' \"$spdx\" > crates/a/src/utils.rs"
expect fail long-file "{ echo \"$spdx\"; for i in \$(seq 500); do echo '// x'; done; } > crates/a/src/long.rs"
expect fail long-fn "{ echo \"$spdx\"; echo 'pub fn g() {'; for i in \$(seq 80); do echo '    let _y = 0;'; done; echo '}'; } > crates/a/src/b.rs"
expect pass fn-at-limit "{ echo \"$spdx\"; echo 'pub fn g() {'; for i in \$(seq 78); do echo '    let _y = 0;'; done; echo '}'; } > crates/a/src/b.rs"
expect pass trait-declarations-then-long-impl "{ echo \"$spdx\"; echo 'pub trait T {'; echo '    fn a(&self) -> u8;'; echo '    fn b('; echo '        &self,'; echo '    ) -> u8;'; echo '}'; echo 'impl S {'; echo '    fn c() {'; for i in \$(seq 78); do echo '        let _y = 0;'; done; echo '    }'; echo '}'; } > crates/a/src/b.rs"
expect fail one-line-fn-then-long-fn "{ echo \"$spdx\"; echo 'fn a() -> u8 { 1 }'; echo 'fn g() {'; for i in \$(seq 80); do echo '    let _y = 0;'; done; echo '}'; } > crates/a/src/b.rs"
expect pass one-line-fn-then-short-fn "{ echo \"$spdx\"; echo 'fn a() -> u8 { 1 }'; for i in \$(seq 90); do echo '// x'; done; echo 'fn g() {'; echo '}'; } > crates/a/src/b.rs"
expect fail no-spdx "echo '//! b' > crates/a/src/b.rs"
expect fail derived-without-copyright "printf '%s\n// Derived from JSBSim src/X.cpp.\n' \"$spdx\" > crates/a/src/b.rs"
expect pass derived-public-domain "printf '%s\n// Derived from JSBSim src/x.cxx. Released into the Public Domain.\n' \"$spdx\" > crates/a/src/b.rs"
expect fail lib-without-doc "printf '%s\npub mod b;\n' \"$spdx\" > crates/a/src/lib.rs"
expect fail lib-with-code "printf '%s\n//! a\npub fn f() {}\n' \"$spdx\" > crates/a/src/lib.rs"
expect fail println "printf '%s\nfn f() {\n    println!(\"x\");\n}\n' \"$spdx\" > crates/a/src/b.rs"
expect pass println-in-unit-tests "mkdir crates/a/src/b && printf '%s\nfn t() {\n    println!(\"x\");\n}\n' \"$spdx\" > crates/a/src/b/tests.rs"
expect fail plus-equals-one "printf '%s\nfn f(id: &mut u32) {\n    *id += 1;\n}\n' \"$spdx\" > crates/a/src/b.rs"
expect fail frame-counter "printf '%s\nfn f(s: &mut S) {\n    s.frame += 1;\n}\n' \"$spdx\" > crates/a/src/b.rs"
expect fail next-id "printf '%s\nfn f(mut next_id: u32) {\n    next_id += 1;\n}\n' \"$spdx\" > crates/a/src/b.rs"
expect pass cursor-increment "printf '%s\nfn f(mut pos: usize) {\n    pos += 1;\n}\n' \"$spdx\" > crates/a/src/b.rs"
expect pass ignores-agent-worktrees "mkdir -p .claude/worktrees/x/src && printf 'fn f() {}\n' > .claude/worktrees/x/src/mod.rs"
expect fail anyhow "printf 'anyhow = \"1\"\n' >> crates/a/Cargo.toml"
expect fail no-workspace-lints "printf '[package]\nname = \"a\"\n' > crates/a/Cargo.toml"
# A crate the workspace excludes repeats its lints; unsafe_code may be deny.
root_lints='[workspace.lints.rust]\nunsafe_code = \"forbid\"\n\n[workspace.lints.clippy]\npanic = \"forbid\"\n'
excluded="printf '[workspace]\nexclude = [\"crates/a\"]\n\n$root_lints' > Cargo.toml"
own_lints() { printf '[package]\nname = "a"\n\n[lints.rust]\nunsafe_code = "%s"\n\n[lints.clippy]\npanic = "%s"\n' "$1" "$2"; }
expect pass excluded-repeats-lints "$excluded && own_lints deny forbid > crates/a/Cargo.toml"
expect fail excluded-relaxes-clippy "$excluded && own_lints forbid deny > crates/a/Cargo.toml"
expect fail excluded-allows-unsafe "$excluded && own_lints allow forbid > crates/a/Cargo.toml"
expect fail member-repeats-lints "printf '[workspace]\n\n$root_lints' > Cargo.toml && own_lints forbid forbid > crates/a/Cargo.toml"
expect fail stray-copy "cp crates/a/src/b.rs 'crates/a/src/b 2.rs'"

echo "check-structure.test: ok"
