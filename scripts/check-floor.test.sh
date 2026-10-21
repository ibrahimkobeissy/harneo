#!/usr/bin/env bash
# Self-test for scripts/check-floor.sh: throwaway git repos built from scripts/fixtures/floor/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
fx="$here/fixtures/floor"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pass=0
failed=0

# new_repo <name>: repo on main with the guard copied in (untracked, so it is out of scope).
new_repo() {
  repo="$tmp/$1"
  mkdir -p "$repo/scripts"
  cp "$here/check-floor.sh" "$repo/scripts/check-floor.sh"
  git -C "$repo" init -q -b main
  git -C "$repo" config user.email t@example.com
  git -C "$repo" config user.name t
  git -C "$repo" config commit.gpgsign false
}
commit() { # commit <message>
  git -C "$repo" add -A -- . ':!scripts/check-floor.sh'
  git -C "$repo" commit -q --allow-empty -m "$1"
}
# expect <pass|fail> <label> [env...]: runs the guard in $repo.
expect() {
  local want=$1 label=$2 got=pass
  shift 2
  if ! env "$@" bash "$repo/scripts/check-floor.sh" >"$tmp/out" 2>&1; then got=fail; fi
  if [[ "$got" == "$want" ]]; then
    pass=$((pass + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: $label (wanted $want, got $got)"
    sed 's/^/    /' "$tmp/out"
  fi
}
has_output() { # has_output <label> <text>
  if grep -Fq -- "$2" "$tmp/out"; then pass=$((pass + 1)); else
    failed=$((failed + 1))
    echo "FAIL: $1 (output lacks: $2)"
  fi
}

n=0
while IFS='|' read -r ext label line; do
  [[ -z "$ext" ]] && continue
  n=$((n + 1))
  file="src.$ext"
  neutral='// neutral'
  [[ "$ext" == rs ]] && neutral='// neutral'

  # added on an uncommitted line: fails, reported as path:line
  new_repo "add-$n"
  printf '%s\n' "$neutral" >"$repo/$file"
  commit base
  printf '%s\n' "$line" >>"$repo/$file"
  expect fail "added: $label" CONSTRAINTS_APPROVED=0
  has_output "report format: $label" "$file:2:"

  # only removed: passes
  new_repo "rm-$n"
  printf '%s\n%s\n' "$neutral" "$line" >"$repo/$file"
  commit base
  printf '%s\n' "$neutral" >"$repo/$file"
  expect pass "removed: $label"

  # unchanged (already in base): passes
  new_repo "keep-$n"
  printf '%s\n%s\n' "$neutral" "$line" >"$repo/$file"
  commit base
  printf '%s\n' '// more' >>"$repo/$file"
  expect pass "unchanged: $label"

  # added in an untracked file: fails
  new_repo "new-$n"
  commit base
  printf '%s\n' "$line" >"$repo/new.$ext"
  expect fail "untracked file: $label"

  # added in a committed change on a work branch: fails
  new_repo "committed-$n"
  printf '%s\n' "$neutral" >"$repo/$file"
  commit base
  git -C "$repo" checkout -q -b work
  printf '%s\n' "$line" >>"$repo/$file"
  commit "add $label"
  expect fail "committed: $label"
done <"$fx/patterns.txt"

# Out-of-scope files are ignored.
new_repo scope
commit base
mkdir -p "$repo/design" "$repo/node_modules/x"
printf '%s\n' '// @ts-ignore' >"$repo/design/a.ts"
printf '%s\n' '// @ts-ignore' >"$repo/node_modules/x/a.js"
printf '%s\n' '// @ts-ignore' >"$repo/notes.md"
expect pass "design/, node_modules/ and Markdown are ignored"

# Deleted test case.
new_repo del
cp "$fx/base.ts.txt" "$repo/a.ts"
cp "$fx/base.rs.txt" "$repo/a.rs"
commit base
sed -i '/^it(/d' "$repo/a.ts"
expect fail "deleted ts test, uncommitted, no Test-change"
has_output "message tells how to add the line" "Test-change: <reason>"
git -C "$repo" checkout -q -- a.ts
sed -i '/^#\[test\]/d' "$repo/a.rs"
expect fail "deleted rust #[test], uncommitted"
git -C "$repo" checkout -q -- a.rs

git -C "$repo" checkout -q -b work
sed -i '/^it(/d' "$repo/a.ts"
commit "Drop obsolete test"
expect fail "deleted test, commit without Test-change"
git -C "$repo" commit -q --allow-empty -m "Note" -m "Test-change:"
expect fail "deleted test, empty Test-change reason"
git -C "$repo" commit -q --allow-empty -m "Why" -m "Test-change: the feature was removed"
expect pass "deleted test, commit with Test-change"

# Skipped test.
new_repo skip
cp "$fx/base.ts.txt" "$repo/a.ts"
commit base
git -C "$repo" checkout -q -b work
sed -i 's/^it(/it.skip(/' "$repo/a.ts"
commit "Skip a test"
expect fail "skipped test, commit without Test-change"
git -C "$repo" commit -q --allow-empty -m "Why" -m "Test-change: flaky on CI, tracked elsewhere"
expect pass "skipped test, commit with Test-change"

# CONSTRAINTS.md.
new_repo cons
cp "$fx/CONSTRAINTS.md.txt" "$repo/CONSTRAINTS.md"
commit base
printf 'weaker\n' >>"$repo/CONSTRAINTS.md"
expect fail "CONSTRAINTS.md changed, not approved"
has_output "approval message" "CONSTRAINTS.md changed: this needs the user's approval"
expect pass "CONSTRAINTS.md changed, approved" CONSTRAINTS_APPROVED=1

# Clean tree.
new_repo clean
cp "$fx/base.ts.txt" "$repo/a.ts"
commit base
expect pass "clean tree"

echo "check-floor self-test: $pass passed, $failed failed"
((failed == 0))
