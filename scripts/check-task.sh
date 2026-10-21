#!/usr/bin/env bash
# check:task - check:fast + Rust format and Clippy, related tests, changed-line coverage.
set -euo pipefail
cd "$(dirname "$0")/.."

bash scripts/check-fast.sh

mapfile -d '' changed < <(bash scripts/changed-files.sh)

ts=()
declare -A touched=()
for f in "${changed[@]}"; do
  case "$f" in
    src/*.ts | src/*.tsx) ts+=("$f") ;;
  esac
  case "$f" in
    Cargo.toml | Cargo.lock)
      touched[harneo-core]=1 touched[harneo-cli]=1 touched[harneo-app]=1
      ;;
    crates/harneo-core/*) touched[harneo-core]=1 ;;
    crates/harneo-cli/*) touched[harneo-cli]=1 ;;
    src-tauri/*) touched[harneo-app]=1 ;;
  esac
done
crates=${!touched[*]}

echo "== cargo fmt --all --check"
cargo fmt --all --check
echo "== cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

# Architecture gates (TS and Rust, ticket #18) go here, before the tests.

if ((${#ts[@]})); then
  echo "== vitest related"
  npx vitest related --run --passWithNoTests "${ts[@]}"
fi
for c in $crates; do
  echo "== cargo test -p $c"
  cargo test -p "$c"
done

bash scripts/coverage.sh "$crates" "${ts[@]}"
echo "check:task OK"
