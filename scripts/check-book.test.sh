#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Proves scripts/check-book.sh rejects each way the manual can drift from
# the code: every case builds a passing tree, plants one violation and
# expects a failure. Uses prebuilt fake API docs and skips the builds.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
checker="$here/check-book.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

book_url='https://luofang34.github.io/jsbsim-rs/'
repo_url='https://github.com/luofang34/jsbsim-rs'

make_tree() {
  local dir="$1"
  mkdir -p "$dir/book/src" "$dir/crates/jsbsim/examples" "$dir/api/jsbsim"
  printf '<h2 id="method.run">run</h2>\n' > "$dir/api/jsbsim/struct.Fdm.html"
  printf '// ANCHOR: run\nfdm.run()?;\n// ANCHOR_END: run\n' > "$dir/crates/jsbsim/examples/demo.rs"
  printf '# Summary\n\n[Intro](intro.md)\n\n- [Guide](guide.md)\n' > "$dir/book/src/SUMMARY.md"
  printf '# Intro\n\n## Getting going\n\nSee [run](api/jsbsim/struct.Fdm.html#method.run).\n' > "$dir/book/src/intro.md"
  cat > "$dir/book/src/guide.md" <<EOF
# Guide

\`\`\`rust
{{#include ../../crates/jsbsim/examples/demo.rs:run}}
\`\`\`

\`\`\`toml
pasted = "fine outside Rust blocks"
\`\`\`

The [example]($repo_url/blob/main/crates/jsbsim/examples/demo.rs), and
[getting going](intro.md#getting-going).
EOF
  printf '# r\n\n[Manual](%s)\n\n```rust\nfdm.run()?;\n```\n' "$book_url" > "$dir/README.md"
  printf '# c\n\n[Manual](%s)\n\n```rust\nlet x = 1;\nfdm.run()?;\n```\n' "$book_url" > "$dir/crates/jsbsim/README.md"
}

expect() {
  local outcome="$1" name="$2" dir="$scratch/$2"
  shift 2
  make_tree "$dir"
  (cd "$dir" && eval "$*")
  if JSBSIM_BOOK_NO_BUILD=1 JSBSIM_BOOK_API_DIR="$dir/api" "$checker" "$dir" > "$dir.log" 2>&1; then
    got=pass
  else
    got=fail
  fi
  if [ "$got" != "$outcome" ]; then
    echo "check-book.test: $name: expected $outcome, got $got" >&2
    cat "$dir.log" >&2
    exit 1
  fi
}

expect pass clean-tree true

# Rust listings are included, never pasted.
expect fail pasted-rust "printf '\`\`\`rust\nlet x = 1;\n\`\`\`\n' >> book/src/guide.md"
# Includes name files and anchors that exist, never line numbers.
expect fail include-missing-file "sed -i.bak 's/demo.rs:run/gone.rs:run/' book/src/guide.md && rm book/src/guide.md.bak"
expect fail include-missing-anchor "sed -i.bak 's/demo.rs:run/demo.rs:walk/' book/src/guide.md && rm book/src/guide.md.bak"
expect fail include-by-lines "sed -i.bak 's/demo.rs:run/demo.rs:1:2/' book/src/guide.md && rm book/src/guide.md.bak"
# Repository links name paths that exist, and no line numbers.
expect fail repo-missing-path "sed -i.bak 's#examples/demo.rs)#examples/gone.rs)#' book/src/guide.md && rm book/src/guide.md.bak"
expect fail repo-line-number "sed -i.bak 's#examples/demo.rs)#examples/demo.rs\#L3)#' book/src/guide.md && rm book/src/guide.md.bak"
# API links name generated pages and anchors.
expect fail api-missing-page "sed -i.bak 's/struct.Fdm.html/struct.Gone.html/' book/src/intro.md && rm book/src/intro.md.bak"
expect fail api-missing-anchor "sed -i.bak 's/method.run/method.walk/' book/src/intro.md && rm book/src/intro.md.bak"
# Chapter links name headings that exist.
expect fail chapter-missing-heading "sed -i.bak 's/intro.md#getting-going/intro.md#going-away/' book/src/guide.md && rm book/src/guide.md.bak"
# SUMMARY.md and the chapters agree.
expect fail orphan-chapter "printf '# Orphan\n' > book/src/orphan.md"
expect fail summary-missing-chapter "printf -- '- [Gone](gone.md)\n' >> book/src/SUMMARY.md"
expect fail nested-chapter "mkdir book/src/part && printf '# Nested\n' > book/src/part/nested.md && printf -- '- [Nested](part/nested.md)\n' >> book/src/SUMMARY.md"
# The READMEs link the manual, and the quickstart is doc-tested.
expect fail readme-without-link "sed -i.bak 's#$book_url#https://example.com/#' README.md && rm README.md.bak"
expect fail crate-readme-without-link "sed -i.bak 's#$book_url#https://example.com/#' crates/jsbsim/README.md && rm crates/jsbsim/README.md.bak"
expect fail quickstart-untested "sed -i.bak 's/fdm.run()?;/fdm.walk()?;/' README.md && rm README.md.bak"

echo "check-book.test: ok"
