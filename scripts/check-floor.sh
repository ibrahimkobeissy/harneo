#!/usr/bin/env bash
# check-floor - the Floor rules of CONSTRAINTS.md, checked on the ADDED lines of the diff
# against main (committed since the merge-base, staged, unstaged and untracked).
# Prints `path:line: <rule>` per hit and exits 1; exits 0 when clean.
# Test-change gate: removed test cases and added skipped/focused tests pass only when a
# commit message in merge-base..HEAD has a `Test-change: <reason>` line.
set -euo pipefail
cd "$(dirname "$0")/.."

base=$(git merge-base HEAD main 2>/dev/null || git rev-parse HEAD)
fail=0

# CONSTRAINTS.md must not be weakened without the user's approval.
if [[ "${CONSTRAINTS_APPROVED:-}" != "1" ]]; then
  if [[ -n "$(git diff --name-only "$base" -- CONSTRAINTS.md)" ||
    -n "$(git ls-files --others --exclude-standard -- CONSTRAINTS.md)" ]]; then
    echo "CONSTRAINTS.md:1: CONSTRAINTS.md changed: this needs the user's approval"
    fail=1
  fi
fi

tc=0
if git log --format=%B "$base..HEAD" 2>/dev/null |
  grep -Eq '^Test-change:[[:space:]]*[^[:space:]]'; then
  tc=1
fi

read -r -d '' prog <<'AWK' || true
function inscope(p) {
  if (p !~ /\.(rs|ts|tsx|js|jsx|mjs|cjs|sh)$/) return 0
  if (p ~ /^(design|\.impeccable)\//) return 0
  if (p ~ /(^|\/)(node_modules|target|dist)\//) return 0
  if (p == "scripts/check-floor.sh" || p == "scripts/check-floor.test.sh") return 0
  if (p ~ /^scripts\/fixtures\/floor\//) return 0
  return 1
}
function hit(msg) { printf "%s:%d: %s\n", file, nl, msg }
function gated(msg) { if (!tc) hit(msg " (add a 'Test-change: <reason>' line to a commit message)") }
function is_skip(t) {
  return t ~ /(^|[^[:alnum:]_.])(it|test|describe)\.skip/ ||
    t ~ /(^|[^[:alnum:]_])(xit|xdescribe)\(/ ||
    t ~ /\.only\(/ || t ~ /#\[ignore([]= ]|$)/
}
/^diff --git / { inhunk = 0; file = ""; oldf = ""; next }
!inhunk && /^--- / { oldf = substr($0, 5); sub(/^a\//, "", oldf); next }
!inhunk && /^\+\+\+ / {
  file = substr($0, 5); sub(/^b\//, "", file)
  if (file == "/dev/null") file = oldf
  next
}
/^@@ / {
  inhunk = 1
  s = $0; sub(/^@@ -/, "", s); ol = s + 0
  match(s, /\+[0-9]+/); nl = substr(s, RSTART + 1, RLENGTH - 1) + 0
  next
}
inhunk && /^\\/ { next }
inhunk && /^-/ {
  t = substr($0, 2)
  if (inscope(file) && !is_skip(t) &&
      (t ~ /#\[test\]/ || t ~ /(^|[^[:alnum:]_.])(it|test|describe)(\.[a-z]+)?\(/)) {
    saved = nl; nl = ol
    gated("removed test case"); nl = saved
  }
  ol++; next
}
inhunk && /^\+/ {
  t = substr($0, 2)
  if (inscope(file)) {
    if (t ~ /@ts-ignore/) hit("suppression: @ts-ignore")
    if (t ~ /@ts-nocheck/) hit("suppression: @ts-nocheck")
    if (t ~ /@ts-expect-error/) hit("suppression: @ts-expect-error")
    if (t ~ /eslint-disable/) hit("suppression: eslint-disable")
    if (t ~ /#!?\[allow\(/) hit("suppression: Rust allow attribute")
    if (t ~ /todo!\(/) hit("stub: todo!()")
    if (t ~ /unimplemented!\(/) hit("stub: unimplemented!()")
    if (t ~ /throw new Error\(["'`]Not implemented/) hit("stub: throw new Error(\"Not implemented\")")
    if (t ~ /catch[[:space:]]*(\([^)]*\))?[[:space:]]*\{[[:space:]]*\}/) hit("stub: empty catch block")
    if (t ~ /(^|[^[:alnum:]_.])(it|test|describe)\.skip/ || t ~ /(^|[^[:alnum:]_])(xit|xdescribe)\(/)
      gated("skipped test")
    if (t ~ /\.only\(/) gated("focused test (.only)")
    if (t ~ /#\[ignore([]= ]|$)/) gated("ignored test (#[ignore])")
  }
  nl++; next
}
inhunk && /^ / { ol++; nl++ }
AWK

run_awk() { awk -v tc="$tc" "$prog"; }

out=$(
  {
    git -c core.quotepath=off diff --src-prefix=a/ --dst-prefix=b/ -U0 --no-color "$base" --
    while IFS= read -r -d '' f; do
      git -c core.quotepath=off diff --no-index --src-prefix=a/ --dst-prefix=b/ -U0 --no-color -- /dev/null "$f" || true
    done < <(git ls-files -z --others --exclude-standard)
  } | run_awk
)

if [[ -n "$out" ]]; then
  echo "$out"
  fail=1
fi

if ((fail)); then
  echo "check-floor: FAILED (see CONSTRAINTS.md, Floor)" >&2
  exit 1
fi
echo "check-floor OK"
