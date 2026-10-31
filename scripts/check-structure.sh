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

# Build output, git data and agent worktrees are not part of the tree.
find_tree() {
  find . \( -path ./target -o -path ./.claude -o -path ./.git \) -prune -o "$@" -print
}

# Files ported from GeographicLib (MIT) declare both licences and carry its
# notice, LICENSES/GeographicLib-MIT.txt, line for line in their header.
# Files ported from ARM optimized-routines (MIT OR Apache-2.0 WITH
# LLVM-exception, LICENSES/optimized-routines.txt) declare that choice and
# carry the MIT section of its licence the same way.
mit_notice=LICENSES/GeographicLib-MIT.txt
arm_licence=LICENSES/optimized-routines.txt
spdx_lgpl='// SPDX-License-Identifier: LGPL-2.1-or-later'
spdx_lgpl_mit='// SPDX-License-Identifier: LGPL-2.1-or-later AND MIT'
spdx_lgpl_arm='// SPDX-License-Identifier: LGPL-2.1-or-later AND (MIT OR Apache-2.0 WITH LLVM-exception)'

# require_notice <file> <licence name> <notice file> [sed range]: every
# non-blank line of the notice (or of its section the range selects)
# appears in the file's comments.
require_notice() {
  local file="$1" name="$2" notice="$3" range="${4:-1,\$}"
  if [ ! -f "$notice" ]; then
    fail "$file: $name, but $notice is missing"
    return
  fi
  local text missing
  text=$(sed -n 's#^//[[:space:]]*##p' "$file")
  missing=$(sed -n "${range}p" "$notice" | grep -v '^[[:space:]]*$' | while IFS= read -r line; do
    printf '%s\n' "$text" | grep -qxF -e "$line" || echo "$line"
  done)
  if [ -n "$missing" ]; then
    fail "$file: $name without the full MIT notice (first missing line: $(echo "$missing" | sed -n 1p))"
  fi
}

check_licence() {
  local file="$1"
  if grep -qxF "$spdx_lgpl_mit" "$file"; then
    if ! grep -q 'Derived from GeographicLib' "$file" || ! grep -q 'Charles Karney' "$file"; then
      fail "$file: LGPL-2.1-or-later AND MIT without a 'Derived from GeographicLib' line and Charles Karney's copyright"
    fi
    require_notice "$file" 'LGPL-2.1-or-later AND MIT' "$mit_notice"
  elif grep -qxF "$spdx_lgpl_arm" "$file"; then
    if ! grep -q 'Derived from ARM optimized-routines' "$file" || ! grep -q 'Arm Limited' "$file" \
      || ! grep -qE 'commit [0-9a-f]{40}' "$file"; then
      fail "$file: optimized-routines licence without a 'Derived from ARM optimized-routines' line, its commit and Arm Limited's copyright"
    fi
    require_notice "$file" 'optimized-routines licence' "$arm_licence" '/^MIT License$/,/^SOFTWARE\.$/'
  elif grep -qxF "$spdx_lgpl" "$file"; then
    if grep -q 'Permission is hereby granted, free of charge' "$file"; then
      fail "$file: carries the MIT notice, so its SPDX line must be 'LGPL-2.1-or-later AND MIT'"
    fi
  else
    fail "$file: missing SPDX-License-Identifier line"
  fi
}

rust_files() {
  find_tree -name '*.rs' -type f | sort
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
  check_licence "$file"
  if grep -q 'Derived from JSBSim' "$file" && ! grep -q -E 'Copyright|Public Domain' "$file"; then
    fail "$file: derived from JSBSim but carries no copyright or public-domain line"
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
  first_doc=$(grep -v '^// SPDX-License-Identifier' "$lib" | sed -n 1p)
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
  # Monotonic counters (IDs, generations, epochs, frames) must wrap
  # explicitly; parser cursors and loop indices are not counters.
  if grep -n -i -E '(id|ids|count|counter|ctr|gen|generation|epoch|seq|serial|frame|frames|tick|ticks)\b[])]* \+= 1\b' "$file"; then
    fail "$file: counters use wrapping_add(1)"
  fi
  if grep -n -E 'process::exit|lazy_static!' "$file"; then
    fail "$file: no process::exit or lazy_static!"
  fi
done < <(production_files)

if grep -rn --include='*.rs' --include='Cargo.toml' -E '\banyhow\b' --exclude-dir=target --exclude-dir=.claude . ; then
  fail "anyhow is not used in this workspace; use thiserror"
fi

# `<table>.<lint>="<level>"`, one per line, from the [<prefix>rust] and
# [<prefix>clippy] sections of a manifest.
lint_levels() {
  awk -v pre="$2" '
    /^\[/ { section = $0; next }
    section == "[" pre "rust]" || section == "[" pre "clippy]" {
      gsub(/[[:space:]]/, "")
      if ($0 ~ /=/) print substr(section, length(pre) + 2, length(section) - length(pre) - 2) "." $0
    }
  ' "$1" | sort
}

# A crate outside the workspace (in its `exclude`) repeats the workspace
# lint table at the same levels; only unsafe_code may drop to deny, for FFI.
while IFS= read -r manifest; do
  if grep -A1 '^\[lints\]' "$manifest" | grep -q '^workspace = true'; then
    continue
  fi
  dir=$(dirname "${manifest#./}")
  if [ ! -f Cargo.toml ] || ! grep -E '^exclude = ' Cargo.toml | grep -qF "\"$dir\""; then
    fail "$manifest: must inherit workspace lints ([lints] workspace = true)"
    continue
  fi
  missing=$(LC_ALL=C comm -23 <(lint_levels Cargo.toml workspace.lints.) \
    <(lint_levels "$manifest" lints. | sed 's/^rust\.unsafe_code="deny"$/rust.unsafe_code="forbid"/'))
  if [ -n "$missing" ]; then
    fail "$manifest: outside the workspace, so it must repeat the workspace lints; missing $(echo "$missing" | tr '\n' ' ')"
  fi
done < <(find_tree -name Cargo.toml -type f | grep -v '^\./Cargo.toml$')

# Under pipefail a pipeline fails when head exits while its writer is still
# writing (SIGPIPE or EPIPE), so whether it fails depends on output size and
# timing. sed -n 1p reads all of its input.
shell_dirs=()
for dir in scripts .github; do [ -d "$dir" ] && shell_dirs+=("$dir"); done
if [ "${#shell_dirs[@]}" -gt 0 ] &&
    find "${shell_dirs[@]}" \( -name '*.sh' -o -name '*.yml' -o -name '*.yaml' \) -type f |
    xargs grep -n -E '\|[[:space:]]*head\b' /dev/null; then
  fail "take the first line with sed -n 1p, not a pipe into head"
fi

if find . -path ./target -prune -o -path ./.git -prune -o -path ./.claude -prune -o \
    \( -name '* 2.*' -o -name '*.bak' -o -name '*.orig' -o -name '*~' \) -print | grep .; then
  fail "stray backup files"
fi

if [ "$failures" -gt 0 ]; then
  echo "check-structure: $failures problem(s)" >&2
  exit 1
fi
echo "check-structure: ok"
