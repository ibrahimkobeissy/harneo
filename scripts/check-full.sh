#!/usr/bin/env bash
# check:full - check:task + full test suites, dependency and code scans, coverage ratchet.
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in cargo osv-scanner jq semgrep; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "check:full: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

bash scripts/check-task.sh

echo "== vitest run"
npx vitest run --passWithNoTests
echo "== cargo test --workspace"
cargo test --workspace

echo "== dependency scan"
bash scripts/check-deps.sh
echo "== semgrep"
semgrep scan --config p/default --config p/rust --config p/typescript --severity ERROR --error

bash scripts/coverage-ratchet.sh
echo "check:full OK"
