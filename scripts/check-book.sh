#!/usr/bin/env bash
# SPDX-License-Identifier: LGPL-2.1-or-later
# Builds the user manual (book/) with the jsbsim API docs under its api/
# directory, as .github/workflows/book.yml deploys it, and fails when the
# manual could drift from the code:
# - a Rust code block holds anything but {{#include}} lines, so every
#   snippet is a crates/jsbsim/examples file that CI compiles and runs;
# - an {{#include}} names a missing file or anchor (mdbook only logs it
#   and leaves the block empty);
# - a link into this repository names a missing path or a line number
#   (lines move; link the file, or the item's API page);
# - a link to a chapter's #heading names a heading that chapter lacks;
# - an api/ link names a page or #anchor the generated rustdoc lacks;
# - a chapter is missing from SUMMARY.md, or SUMMARY.md names a missing
#   chapter;
# - README.md does not link to the manual, or its Rust block is not, word
#   for word, part of crates/jsbsim/README.md, whose blocks run as doc tests.
# Usage:
#   scripts/check-book.sh [repo-root]
# Needs mdbook (`cargo install --locked mdbook@0.5.4`). JSBSIM_BOOK_API_DIR
# names prebuilt API docs to check against and JSBSIM_BOOK_NO_BUILD=1 skips
# both builds (for scripts/check-book.test.sh).
set -euo pipefail

root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

repo_url='https://github.com/luofang34/jsbsim-rs'
book_url='https://luofang34.github.io/jsbsim-rs/'
src=book/src
site=target/book
docflags="-D missing_docs -D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links"
failures=0
fail() {
  echo "check-book: $*" >&2
  failures=$((failures + 1))
}

api="${JSBSIM_BOOK_API_DIR:-}"
if [ -z "${JSBSIM_BOOK_NO_BUILD:-}" ]; then
  if ! command -v mdbook > /dev/null; then
    echo "check-book: mdbook is not installed; run" >&2
    echo "  cargo install --locked mdbook@0.5.4" >&2
    exit 1
  fi
  # The same build as scripts/ci.sh's doc-dependent gate, so it is reused.
  RUSTDOCFLAGS="$docflags" cargo doc -q -p jsbsim --features fs --no-deps --target-dir target/doc-default
  api=target/doc-default/doc
fi
if [ -z "$api" ] || [ ! -d "$api" ]; then
  echo "check-book: no API docs to check against (${api:-unset})" >&2
  exit 1
fi

chapters=$(cd "$src" && find . -name '*.md' ! -name SUMMARY.md | sed 's#^\./##' | sort)
listed=$(sed -n 's/^ *[-*] *\[[^]]*\](\([^)]*\.md\)).*$/\1/p; s/^\[[^]]*\](\([^)]*\.md\)) *$/\1/p' "$src/SUMMARY.md" | sort -u)
while IFS= read -r chapter; do
  [ -z "$chapter" ] && continue
  printf '%s\n' "$listed" | grep -qxF -e "$chapter" || fail "$src/$chapter is not in SUMMARY.md"
done <<< "$chapters"
while IFS= read -r chapter; do
  [ -z "$chapter" ] && continue
  [ -f "$src/$chapter" ] || fail "SUMMARY.md names $chapter, which does not exist"
done <<< "$listed"

# check_includes <markdown file>
check_includes() {
  local md="$1" dir spec file anchor
  dir=$(dirname "$md")
  while IFS= read -r spec; do
    file=${spec%%:*}
    anchor=
    [ "$file" != "$spec" ] && anchor=${spec#*:}
    if [ ! -f "$dir/$file" ]; then
      fail "$md: includes $file, which does not exist"
    elif [ -n "$anchor" ] && ! [[ "$anchor" =~ ^[0-9:]*$ ]]; then
      grep -q "ANCHOR: $anchor\$" "$dir/$file" && grep -q "ANCHOR_END: $anchor\$" "$dir/$file" \
        || fail "$md: includes $file:$anchor, but that file has no anchor $anchor"
    elif [ -n "$anchor" ]; then
      fail "$md: includes $file by line numbers ($anchor); add an ANCHOR to it"
    fi
  done < <(grep -oE '\{\{#(include|rustdoc_include) [^}]+\}\}' "$md" | sed -E 's/^\{\{#[a-z_]+ //; s/\}\}$//')
}

# check_rust_blocks <markdown file>: inside ```rust fences, only includes.
check_rust_blocks() {
  awk -v md="$1" '
    /^```/ {
      if (fence) { fence = 0; next }
      fence = 1; rust = ($0 ~ /^```rust/); next
    }
    fence && rust && $0 !~ /^\{\{#(include|rustdoc_include) [^}]+\}\}$/ && $0 !~ /^ *$/ {
      printf "%s:%d: Rust code pasted into the manual; include it from an example\n", md, NR
    }
  ' "$1"
}

# check_links <markdown file>
check_links() {
  local md="$1" link path frag page
  while IFS= read -r link; do
    path=${link#"$repo_url"/}
    path=${path#blob/main/}
    path=${path#tree/main/}
    frag=
    [[ "$path" == *"#"* ]] && frag=${path#*#} && path=${path%%#*}
    if [[ "$frag" =~ ^L[0-9] ]]; then
      fail "$md: links to a line number ($link); lines move, link the file or its API page"
    elif [[ "$link" == "$repo_url"/blob/* || "$link" == "$repo_url"/tree/* ]] && [ ! -e "$path" ]; then
      fail "$md: links to $path, which does not exist"
    fi
  done < <(grep -oE "$repo_url/(blob|tree)/[^)\" >]+" "$md" || true)
  while IFS= read -r link; do
    page=${link#api/}
    frag=
    [[ "$page" == *"#"* ]] && frag=${page#*#} && page=${page%%#*}
    if [ ! -f "$api/$page" ]; then
      fail "$md: links to api/$page, which rustdoc does not generate"
    elif [ -n "$frag" ] && ! grep -q "id=\"$frag\"" "$api/$page"; then
      fail "$md: links to api/$page#$frag, but that page has no anchor $frag"
    fi
  done < <(grep -oE '\]\(api/[^) ]+\)' "$md" | sed -E 's/^\]\(//; s/\)$//' || true)
  while IFS= read -r link; do
    page=${link%%#*}
    frag=${link#*#}
    [ -z "$page" ] && page=$(basename "$md")
    if [ ! -f "$src/$page" ]; then
      fail "$md: links to $page, which does not exist"
    elif ! heading_slugs "$src/$page" | grep -qxF -e "$frag"; then
      fail "$md: links to $page#$frag, but $page has no heading with that anchor"
    fi
  done < <(grep -oE '\]\([a-z0-9-]*\.md#[^) ]+\)|\]\(#[^) ]+\)' "$md" | sed -E 's/^\]\(//; s/\)$//' || true)
}

# heading_slugs <markdown file>: the anchors mdbook gives its headings.
heading_slugs() {
  awk '/^```/ { f = !f } !f && /^#+ / { sub(/^#+ +/, ""); print }' "$1" \
    | tr '[:upper:]' '[:lower:]' | sed -E 's/`//g; s/[^a-z0-9 _-]//g; s/ /-/g'
}

while IFS= read -r chapter; do
  [ -z "$chapter" ] && continue
  md="$src/$chapter"
  case "$chapter" in
    */*) fail "$md: chapters live directly in $src, so api/ links resolve from every page" ;;
  esac
  check_includes "$md"
  check_links "$md"
  while IFS= read -r problem; do
    fail "$problem"
  done < <(check_rust_blocks "$md")
done <<< "$chapters"

for readme in README.md crates/jsbsim/README.md; do
  grep -qF "$book_url" "$readme" || fail "$readme does not link to the manual at $book_url"
done
quickstart=$(awk '/^```rust/ { f = 1; next } /^```/ { f = 0 } f' README.md)
if [ -z "$quickstart" ]; then
  fail "README.md has no Rust quickstart block"
elif [[ "$(cat crates/jsbsim/README.md)" != *"$quickstart"* ]]; then
  fail "README.md's Rust block is not part of crates/jsbsim/README.md, whose blocks run as doc tests"
fi

if [ -z "${JSBSIM_BOOK_NO_BUILD:-}" ] && [ "$failures" -eq 0 ]; then
  rm -rf "$site"
  mdbook build book 2> "target/book-build.log" || { cat target/book-build.log >&2; fail "mdbook build failed"; }
  if grep -E 'ERROR|WARN' target/book-build.log >&2; then
    fail "mdbook reported the problems above"
  fi
  cp -R "$api" "$site/api"
fi

if [ "$failures" -gt 0 ]; then
  echo "check-book: $failures problem(s)" >&2
  exit 1
fi
summary="$(printf '%s\n' "$chapters" | wc -l | tr -d ' ') chapters"
[ -z "${JSBSIM_BOOK_NO_BUILD:-}" ] && summary="$summary; site in $site"
echo "check-book: ok ($summary)"
