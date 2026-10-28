#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Keeps the executive's borrow splitting in place. Model code evaluates
# through `Env::eval` / `Env::cache_value` with `&Models` while it borrows
# its own load-time lists, so it never needs to copy those lists or move a
# model out of `Models` to get past the borrow checker. Usage:
#   scripts/check-fdm-borrows.sh [repo-root]
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

src=crates/jsbsim/src
failures=0

fail() {
  echo "check-fdm-borrows: $*" >&2
  failures=$((failures + 1))
}

# The executive's production sources; unit tests may clone freely.
fdm_files() {
  { [ -f "$src/fdm.rs" ] && echo "$src/fdm.rs"; find "$src/fdm" -name '*.rs' -type f; } |
    grep -v '/tests\.rs$' | sort
}

while IFS= read -r file; do
  # Moving a whole model out of `Models` so a `&mut self` helper can run.
  if grep -n -E 'mem::(take|replace|swap)\(\s*&mut\s+((self|fdm)\.)?models\.[a-z_]+\s*[,)]' "$file"; then
    fail "$file: borrow the model and self.env disjointly instead of moving it out of Models"
  fi
  # Copying a model's function or component list to iterate it.
  if grep -n -E '\.(pre|post|local_properties|functions|functions_at_cg)\.(clone|to_vec|to_owned)\(\)' "$file"; then
    fail "$file: iterate the function list by reference while evaluating through self.env"
  fi
  if grep -n -E '\.(channels|components|forces)\[[^]]*\]\.(clone|to_owned)\(\)' "$file"; then
    fail "$file: borrow the component while evaluating; copy out only Copy ids"
  fi
  # Hand-built getters bypass `Env`, which is what keeps the borrows disjoint.
  case "$file" in
    "$src/fdm/env.rs") ;;
    *)
      if grep -n -E 'ModelGetters \{' "$file"; then
        fail "$file: evaluate through Env::eval or Env::cache_value"
      fi
      ;;
  esac
done < <(fdm_files)

if [ "$failures" -gt 0 ]; then
  echo "check-fdm-borrows: $failures problem(s)" >&2
  exit 1
fi
echo "check-fdm-borrows: ok"
