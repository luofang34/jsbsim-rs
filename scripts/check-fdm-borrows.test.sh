#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-fdm-borrows.sh rejects each workaround: every case
# copies the real executive sources, plants one line and expects the
# outcome.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
checker="$here/check-fdm-borrows.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

src=crates/jsbsim/src

expect() {
  local outcome="$1" name="$2" file="$3" line="$4" dir="$scratch/$2"
  mkdir -p "$dir/$src"
  cp "$repo/$src/fdm.rs" "$dir/$src/fdm.rs"
  cp -R "$repo/$src/fdm" "$dir/$src/fdm"
  if [ -n "$file" ]; then
    printf '%s\n' "$line" >> "$dir/$src/$file"
  fi
  if "$checker" "$dir" > "$dir.log" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" != "$outcome" ]; then
    echo "check-fdm-borrows.test: $name: expected $outcome, got $got" >&2
    cat "$dir.log" >&2
    exit 1
  fi
}

expect pass clean-tree "" ""
expect fail take-model fdm/models_run.rs '        let mut fcs = std::mem::take(&mut self.models.fcs);'
expect fail replace-model fdm/load.rs '        let a = mem::replace(&mut self.models.aerodynamics, Default::default());'
expect fail take-model-in-fdm-rs fdm.rs '        let g = std::mem::take(&mut fdm.models.ground_reactions);'
expect pass drain-a-queue fdm/run.rs '        let requests = std::mem::take(&mut self.models.exec.requests);'
expect fail clone-pre fdm/models_run.rs '        let ids = if pre { mf.pre.clone() } else { mf.post.clone() };'
expect fail clone-aero-functions fdm/models_run.rs '            aero.functions_at_cg.clone(),'
expect fail clone-local-properties fdm/run.rs '        for (n, v) in mf.local_properties.clone() {'
expect fail clone-component fdm/models_run.rs '            let component = self.models.fcs.channels[c].components[i].clone();'
expect fail hand-built-getters fdm/script.rs '        let getters = ModelGetters {'
expect pass getters-in-env fdm/env.rs '        let getters = ModelGetters {'
expect pass clone-in-unit-tests fdm/tests.rs '    let ids = mf.pre.clone();'

echo "check-fdm-borrows.test: ok"
