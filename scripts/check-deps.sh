#!/usr/bin/env bash
# Dependency gate (CONSTRAINTS.md "Security: dependencies"): fail on any advisory at
# HIGH or above (CVSS >= 7.0); print lower and unscored advisories as warnings.
# CHECK_DEPS_INPUT=<file> reads a saved `osv-scanner --format json` result instead of
# scanning (used by the self-test).
set -euo pipefail
cd "$(dirname "$0")/.."

for tool in osv-scanner jq; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "check-deps: '$tool' is not installed. Install $tool (see BUILDING.md)." >&2
    exit 1
  }
done

if [[ -n "${CHECK_DEPS_INPUT:-}" ]]; then
  json=$(cat "$CHECK_DEPS_INPUT")
else
  # Name the lockfiles: scanning "." misses them inside git worktrees (.gitignore'd path).
  # osv-scanner exits 1 when it finds anything; the severity decision is made below.
  json=$(osv-scanner scan source -L Cargo.lock -L package-lock.json --format json 2>/dev/null || true)
fi

rows=$(printf '%s' "$json" | jq -r '
  (.results // [])[] | (.source.path | split("/") | last) as $file
  | .packages[] | .package as $p | .groups[]
  | [($file), $p.name, $p.version, (.ids | join(",")), (.max_severity // "")] | @tsv')

high=0
while IFS=$'\t' read -r file name version ids severity; do
  [[ -n "$name" ]] || continue
  label="${severity:-unscored}"
  if [[ -n "$severity" ]] && awk -v s="$severity" 'BEGIN { exit !(s >= 7.0) }'; then
    echo "check-deps: HIGH $name $version ($file) $ids, CVSS $label" >&2
    high=1
  else
    echo "check-deps: warning: $name $version ($file) $ids, CVSS $label (below high)"
  fi
done <<<"$rows"

((high == 0)) || { echo "check-deps: FAILED (advisories at high or above)" >&2; exit 1; }
echo "check-deps OK (nothing at high or above)"
