#!/usr/bin/env bash
# security-guardrails: install a gitleaks pre-commit hook in the current repo.
#
# gitleaks scans what is about to be committed and refuses the commit if it
# finds a secret (API key, token, private key...). Run from anywhere inside
# the repository:
#
#     bash .claude/skills/security-guardrails/scripts/install-gitleaks-hook.sh
#
# Idempotent. An existing pre-commit hook is kept and still runs after the
# scan. A GitButler-managed hook is left alone and the scan goes into
# pre-commit-user, which GitButler runs first.
set -euo pipefail

MARKER="# security-guardrails: gitleaks"

git rev-parse --git-dir >/dev/null 2>&1 || { echo "Not inside a git repository." >&2; exit 1; }

hooks=$(git rev-parse --git-path hooks)
case "$hooks" in /*) ;; *) hooks="$PWD/$hooks" ;; esac
mkdir -p "$hooks"

target="$hooks/pre-commit"
if [ -f "$target" ] && grep -q "GITBUTLER_MANAGED_HOOK" "$target"; then
  target="$hooks/pre-commit-user"
fi

if [ -f "$target" ] && grep -q "$MARKER" "$target"; then
  echo "gitleaks pre-commit hook already installed ($target)."
  exit 0
fi

if [ -e "$target" ]; then
  mv "$target" "$target.before-gitleaks"
  echo "Kept the existing hook as $(basename "$target").before-gitleaks; it still runs after the scan."
fi

cat > "$target" <<'HOOK'
#!/bin/sh
# security-guardrails: gitleaks
# Refuses a commit whose staged changes contain a secret.
if command -v gitleaks >/dev/null 2>&1; then
  if ! gitleaks git --pre-commit --staged --redact --no-banner --verbose; then
    echo "" >&2
    echo "gitleaks found a secret in the staged changes. Commit refused." >&2
    echo "Remove the secret (and rotate it if it was real), then commit again." >&2
    exit 1
  fi
else
  echo "warning: gitleaks is not installed; secret scan skipped." >&2
fi
if [ -x "$0.before-gitleaks" ]; then
  exec "$0.before-gitleaks" "$@"
fi
exit 0
HOOK
chmod +x "$target"

if ! command -v gitleaks >/dev/null 2>&1; then
  echo "Note: gitleaks is not installed on this machine, so the hook will only warn." >&2
fi
echo "Installed the gitleaks pre-commit hook ($target)."
