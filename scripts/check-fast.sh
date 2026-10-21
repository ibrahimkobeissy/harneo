#!/usr/bin/env bash
# check:fast - types, TS lint and format on changed files, cargo check, leak scan.
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in cargo gitleaks; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "check:fast: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

# Changed files: committed since the merge-base with main, staged, unstaged, untracked.
base=$(git merge-base HEAD main 2>/dev/null || git rev-parse HEAD)
changed=()
while IFS= read -r -d '' f; do
  [[ -f "$f" ]] && changed+=("$f")
done < <(
  {
    git diff --name-only -z --diff-filter=d "$base"
    git ls-files -z --others --exclude-standard
  } | sort -zu
)

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

echo "== cargo check --workspace"
cargo check --workspace

echo "== gitleaks"
gitleaks dir . --redact --no-banner
echo "check:fast OK"
