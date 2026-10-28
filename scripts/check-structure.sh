#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Structural rules that rustc and Clippy do not enforce. Usage:
#   scripts/check-structure.sh [repo-root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

max_file_lines=500
max_fn_lines=80
max_lib_lines=100
failures=0

fail() {
  echo "check-structure: $*" >&2
  failures=$((failures + 1))
}

rust_files() {
  find . -path ./target -prune -o -name '*.rs' -type f -print | sort
}

# Production sources: everything except unit-test modules and integration tests.
production_files() {
  rust_files | grep -v -E '(/tests\.rs$|/tests/)' || true
}

while IFS= read -r file; do
  base="$(basename "$file")"
  case "$base" in
    mod.rs) fail "$file: use foo.rs + foo/, not mod.rs" ;;
    utils.rs | helpers.rs | common.rs) fail "$file: name modules by domain" ;;
  esac
  lines=$(wc -l < "$file" | tr -d ' ')
  if [ "$lines" -gt "$max_file_lines" ]; then
    fail "$file: $lines lines (max $max_file_lines)"
  fi
  if ! grep -q '^// SPDX-License-Identifier: LGPL-2.1-or-later$' "$file"; then
    fail "$file: missing SPDX-License-Identifier line"
  fi
  if grep -q 'Derived from JSBSim' "$file" && ! grep -q 'Copyright' "$file"; then
    fail "$file: derived from JSBSim but carries no copyright line"
  fi
  # Function bodies, relying on rustfmt layout: a signature at some indent
  # ends at the first line holding only "}" at that same indent.
  awk -v max="$max_fn_lines" -v file="$file" '
    function indent(s) { match(s, /^ */); return RLENGTH }
    # Declarations (ending in ";") and one-line bodies open no span.
    start == 0 && /^ *(pub(\([a-z]+\))? )?(const )?(async )?fn / && !/;$/ && !/}/ {
      start = NR; depth = indent($0); name = $0; next
    }
    # A multi-line signature that ends in ";" at its own indent is a declaration.
    start > 0 && indent($0) == depth && /;$/ { start = 0; next }
    start > 0 && $0 ~ "^ {" depth "}}$" && indent($0) == depth {
      if (NR - start + 1 > max) {
        sub(/^ */, "", name)
        printf "check-structure: %s:%d: function is %d lines (max %d): %s\n",
          file, start, NR - start + 1, max, name > "/dev/stderr"
        bad = 1
      }
      start = 0
    }
    END { exit bad }
  ' "$file" || failures=$((failures + 1))
done < <(rust_files)

while IFS= read -r lib; do
  lines=$(wc -l < "$lib" | tr -d ' ')
  if [ "$lines" -gt "$max_lib_lines" ]; then
    fail "$lib: $lines lines (max $max_lib_lines)"
  fi
  first_doc=$(grep -v '^// SPDX-License-Identifier' "$lib" | head -n 1)
  case "$first_doc" in
    '//!'*) ;;
    *) fail "$lib: must start with a crate-level //! doc comment" ;;
  esac
  if grep -v -E '^\s*(//|$|pub mod |mod |pub use |#\[cfg)' "$lib" | grep -q .; then
    fail "$lib: only module declarations and re-exports belong in lib.rs"
  fi
done < <(rust_files | grep '/lib\.rs$' || true)

while IFS= read -r file; do
  if grep -n -E '\b(println|eprintln|print|eprint|dbg)!' "$file"; then
    fail "$file: use tracing, not print macros"
  fi
  if grep -n -E '\+= 1\b' "$file"; then
    fail "$file: counters use wrapping_add(1)"
  fi
  if grep -n -E 'process::exit|lazy_static!' "$file"; then
    fail "$file: no process::exit or lazy_static!"
  fi
done < <(production_files)

if grep -rn --include='*.rs' --include='Cargo.toml' -E '\banyhow\b' --exclude-dir=target . ; then
  fail "anyhow is not used in this workspace; use thiserror"
fi

while IFS= read -r manifest; do
  if ! grep -A1 '^\[lints\]' "$manifest" | grep -q '^workspace = true'; then
    fail "$manifest: must inherit workspace lints ([lints] workspace = true)"
  fi
done < <(find . -path ./target -prune -o -name Cargo.toml -type f -print | grep -v '^\./Cargo.toml$')

if find . -path ./target -prune -o -path ./.git -prune -o \
    \( -name '* 2.*' -o -name '*.bak' -o -name '*.orig' -o -name '*~' \) -print | grep .; then
  fail "stray backup files"
fi

if [ "$failures" -gt 0 ]; then
  echo "check-structure: $failures problem(s)" >&2
  exit 1
fi
echo "check-structure: ok"
