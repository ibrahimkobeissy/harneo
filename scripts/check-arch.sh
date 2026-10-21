#!/usr/bin/env bash
# Rust architecture gate: harneo-core must not depend on tauri*, clap, harneo-app or harneo-cli.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v cargo >/dev/null 2>&1 || {
  echo "check-arch: 'cargo' is not installed. Install cargo (see BUILDING.md)." >&2
  exit 1
}

deps=$(cargo tree -p harneo-core -e normal --prefix none)
fail=0
while read -r name _; do
  case "$name" in
    clap | harneo-app | harneo-cli | tauri*)
      echo "check-arch: harneo-core depends on forbidden crate '$name'. Trace it with: cargo tree -p harneo-core -e normal -i $name" >&2
      fail=1
      ;;
  esac
done < <(printf '%s\n' "$deps" | sort -u)

((fail == 0)) || exit 1
echo "check-arch OK"
