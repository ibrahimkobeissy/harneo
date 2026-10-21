#!/usr/bin/env bash
# Changed-line coverage: TS (vitest + v8) and Rust (cargo llvm-cov), gated by diff-cover.
# Usage: coverage.sh "<space-separated touched crates, or empty>" [changed TS/TSX files...]
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in cargo-llvm-cov diff-cover; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "coverage: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

crates=$1
shift
ts_files=("$@")

rm -rf coverage
mkdir -p coverage
reports=()

if ((${#ts_files[@]})); then
  echo "== vitest coverage (related tests)"
  npx vitest related --run --passWithNoTests --coverage "${ts_files[@]}"
  [[ -f coverage/lcov.info ]] && reports+=(coverage/lcov.info)
fi

if [[ -n "$crates" ]]; then
  echo "== cargo llvm-cov ($crates)"
  args=()
  for c in $crates; do args+=(-p "$c"); done
  cargo llvm-cov "${args[@]}" --lcov --output-path coverage/rust.raw.lcov
  # diff-cover needs repo-relative paths; llvm-cov writes absolute ones.
  sed "s#^SF:$PWD/#SF:#" coverage/rust.raw.lcov >coverage/rust.lcov
  reports+=(coverage/rust.lcov)
fi

if ((${#reports[@]} == 0)); then
  echo "coverage: no changed TS or Rust sources, nothing to check"
  exit 0
fi

echo "== diff-cover (changed lines >= 80%)"
diff-cover "${reports[@]}" --compare-branch=main --fail-under=80
