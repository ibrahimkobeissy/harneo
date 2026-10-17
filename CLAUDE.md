# Harneo

Harneo is a harness manager for Claude Code: a Tauri desktop app that keeps a library of AI Artifacts, composes profiles from it, and scaffolds or refits projects from a profile. **Read `CONTEXT.md` before designing or building anything.** It is the product source of truth: artifacts, user journey, feature decisions and constraints.

## Status
- The repo is empty apart from `CONTEXT.md`, `LICENSE` (MIT) and `README.md`. No code, no build commands yet. Add them here once the Tauri scaffold exists.
- Stack decided: Tauri 2. The frontend framework is not decided yet.
- Cross-platform code, built and tested on Linux only. Distribution is source only (see `CONTEXT.md` §4–5).

## Issue tracker
- Tasks are tracked in **GitHub Issues** for `ibrahimkobeissy/harneo`, using the `gh` CLI. Do not write `tasks/todo.md`.
- One issue per task: acceptance criteria and verification steps in the issue body; dependencies as "Blocked by #N".
- Add every new issue to GitHub Project 5 ("Harneo", https://github.com/users/ibrahimkobeissy/projects/5): `gh project item-add 5 --owner ibrahimkobeissy --url <issue-url>`.
- `tasks/plan.md` keeps only the ordered list of issue links, never a duplicate checklist.

## Security Guardrails

Agents operate **inside the project directory**. Treat anything outside it as
off-limits unless the user explicitly asks. The paths below are hard
prohibitions: never `Read`, `Edit`, `Write`, `cat`/`less`/`grep`/copy, or
otherwise access them, and **never echo their contents** into a file, a note, a
web request, or the chat (no exfiltration).

- **Credential & key stores:** `~/.ssh`, `~/.aws`, `~/.gnupg`, `~/.config/gcloud`,
  `~/.azure`, `~/.kube`, `~/.oci`, `~/.docker/config.json`, `~/.netrc`,
  `~/.git-credentials`, `~/.config/gh`, `~/.npmrc`, `~/.pypirc`.
- **Agent tokens:** `~/.config/anthropic`, `~/.claude/.credentials.json`,
  `~/.codex`, `~/.gemini`.
- **Secret files anywhere:** `.env` / `.env.*`, `*.pem`, `*.key`, `*.p12`,
  `*.pfx`, `*.jks`, `*.keystore`, `id_rsa*`, `id_ed25519*`, `id_ecdsa*`,
  `*service-account*.json`, `*credentials*.json`, `secrets.*`, `*.secret`,
  `*.tfstate*`, `.pgpass`, `.my.cnf`, `.vault-token`.
- **Shell/DB history:** `~/.bash_history`, `~/.zsh_history`, `~/.python_history`,
  `~/.psql_history`.

**Enforcement (Claude Code):** `permissions.deny` in `.claude/settings.json`
covers the native Read/Edit/Write tools, and the `PreToolUse` hook
(`guard-sensitive-paths.py`) covers shell commands — including the `cat … `
case that path-based denies cannot stop. If a task genuinely requires touching
one of these paths, **stop and ask the user to do it manually**; do not work
around the guardrail.
