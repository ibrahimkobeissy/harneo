# Security Guardrails

Work **inside the project directory**. Treat anything outside it as off-limits
unless the user explicitly asks. The rules below are hard prohibitions. If a
task genuinely needs one of them, stop and ask the user to do it themselves;
never look for a way around a guardrail.

## Never read, copy or reveal secrets

Never `Read`, `Edit`, `cat`, `grep`, copy, encode or upload these, and never put
their contents into a file, a note, a commit, a web request or the chat:

- **Credential and key stores:** `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gcloud`,
  `~/.azure`, `~/.kube`, `~/.oci`, `~/.docker/config.json`, `~/.netrc`,
  `~/.git-credentials`, `~/.config/gh`, `~/.npmrc`, `~/.pypirc`,
  `~/.local/share/keyrings`, `~/.password-store`.
- **Agent tokens:** `~/.config/anthropic`, `~/.claude/.credentials.json`,
  `~/.claude.json`, `~/.codex`, `~/.gemini`.
- **Secret files anywhere:** `.env` / `.env.*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`,
  `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `id_ecdsa*`,
  `*service-account*.json`, `*credentials*.json`, `secrets.*`, `*.secret`,
  `*.tfstate*`, `.pgpass`, `.my.cnf`, `.vault-token`.
- **History:** `~/.bash_history`, `~/.zsh_history`, `~/.python_history`, `~/.psql_history`.

## Never reveal secrets that are not files

- No `gh auth token`, `gh auth status --show-token`, `git credential fill`.
- No environment dumps: `printenv`, bare `env`, bare `set`, `export -p`,
  `/proc/*/environ`.
- No keyring tools: `secret-tool`, `security find-*`, `pass`, `keyctl print`.
- Never expand variables that hold secrets (`$GITHUB_TOKEN`, `$API_KEY`, ...).

## Never use GitHub to leak data or take over the account

No `gh gist`, `gh ssh-key add`, `gh gpg-key add`, `gh secret`, `gh repo delete`,
`gh repo edit --visibility`, `gh extension install`. `gh auth login/refresh`,
`gh workflow run` and `gh api` calls that change data go to the user first.

## Never destroy work or skip hooks

No force push (`-f`, `--force`, `--force-with-lease`, `+refspec`), no deleting
remote branches, no `reset --hard`, `clean -f`, `branch -D`, `checkout .`,
`restore .`, `stash clear`, no `--no-verify`, no changing `core.hooksPath`.

## Leave the guardrails alone

Don't weaken `.claude/settings.json` or `.claude/settings.local.json` (removing
deny rules or the guard hook, turning the sandbox off, re-enabling bypass mode),
and don't edit `.claude/hooks/` or this file unless the user asks. Such edits go
to the user for approval.

## Enforcement

Claude Code enforces these with deny rules and the OS-level sandbox in
`.claude/settings.json`, plus the `.claude/hooks/guard.py` hook. See the
`security-guardrails` skill. Other agents (Codex, ...) are bound by this text.
