#!/usr/bin/env bash
# Coverage ratchet: whole-project line coverage (Rust, TS) must not fall below the baseline in CONSTRAINTS.md.
# Baselines come from the two "Project coverage" rows of "Measured, not yet enforced" (a "Today" cell like
# `87.4% (2026-10-22, abc1234)`). No percentage in the cell means no baseline yet, which passes.
# Override for tests: RATCHET_BASELINE_RUST=99 RATCHET_BASELINE_TS=99 (percent).
set -euo pipefail
cd "$(dirname "$0")/.."

TOLERANCE=0.1

for tool in cargo-llvm-cov; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "ratchet: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

# baseline <row label> -> first percentage in that row's "Today" cell, or empty.
baseline() {
  grep -F "| $1 |" CONSTRAINTS.md | awk -F'|' '{print $3}' | grep -oE '[0-9]+(\.[0-9]+)?%' | head -n1 | tr -d '%' || true
}

# lcov_pct <file> -> line coverage percent from LF/LH totals.
lcov_pct() {
  awk -F: '/^LF:/{lf+=$2} /^LH:/{lh+=$2} END{if(lf==0){print "0"}else{printf "%.2f", 100*lh/lf}}' "$1"
}

base_rust=${RATCHET_BASELINE_RUST:-$(baseline "Project coverage, Rust")}
base_ts=${RATCHET_BASELINE_TS:-$(baseline "Project coverage, TypeScript")}

mkdir -p coverage
echo "== ratchet: vitest run --coverage"
npx vitest run --passWithNoTests --coverage
cur_ts=$(lcov_pct coverage/lcov.info)

echo "== ratchet: cargo llvm-cov --workspace"
cargo llvm-cov --workspace --lcov --output-path coverage/rust-full.lcov
cur_rust=$(lcov_pct coverage/rust-full.lcov)

fail=0
check() { # name current baseline
  if [[ -z "$3" ]]; then
    echo "ratchet: $1 coverage ${2}%, no baseline yet"
  elif awk -v c="$2" -v b="$3" -v t="$TOLERANCE" 'BEGIN{exit !(c + t < b)}'; then
    echo "ratchet: FAIL $1 coverage ${2}% fell below baseline ${3}%" >&2
    fail=1
  else
    echo "ratchet: $1 coverage ${2}% (baseline ${3}%) ok"
  fi
}
check Rust "$cur_rust" "$base_rust"
check TypeScript "$cur_ts" "$base_ts"
exit $fail
