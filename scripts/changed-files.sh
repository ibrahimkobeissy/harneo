#!/usr/bin/env bash
# Prints the changed files, NUL-separated: committed since the merge-base with main,
# staged, unstaged and untracked. Deleted files are left out. Read with:
#   mapfile -d '' changed < <(bash scripts/changed-files.sh)
set -euo pipefail
cd "$(dirname "$0")/.."

base=$(git merge-base HEAD main 2>/dev/null || git rev-parse HEAD)
{
  git diff --name-only -z --diff-filter=d "$base"
  git ls-files -z --others --exclude-standard
} | sort -zu | while IFS= read -r -d '' f; do
  [[ -f "$f" ]] && printf '%s\0' "$f"
done
exit 0
