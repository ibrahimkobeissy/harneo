#!/usr/bin/env bash
# check:fast - types, TS lint and format on changed files, floor guard, cargo check, leak scan.
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in cargo gitleaks; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "check:fast: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

# Changed files: committed since the merge-base with main, staged, unstaged, untracked.
mapfile -d '' changed < <(bash scripts/changed-files.sh)

lintable=()
formattable=()
for f in "${changed[@]}"; do
  case "$f" in
    *.ts | *.tsx | *.js | *.jsx | *.mjs | *.cjs)
      lintable+=("$f")
      formattable+=("$f")
      ;;
    *.json | *.css | *.html) formattable+=("$f") ;;
  esac
done

echo "== tsc --noEmit"
npx tsc --noEmit

if ((${#lintable[@]})); then
  echo "== eslint (changed files)"
  npx eslint --no-warn-ignored "${lintable[@]}"
fi
if ((${#formattable[@]})); then
  echo "== prettier --check (changed files)"
  npx prettier --check --ignore-unknown "${formattable[@]}"
fi

echo "== check-floor"
bash scripts/check-floor.sh

echo "== setup self-test"
bash scripts/setup.test.sh

echo "== cargo check --workspace"
cargo check --workspace

echo "== gitleaks"
gitleaks dir . --redact --no-banner
echo "check:fast OK"
