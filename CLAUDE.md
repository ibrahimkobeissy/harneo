# Harneo

Harneo is a harness manager for Claude Code: a Tauri desktop app that keeps a library of AI Artifacts, composes profiles from it, and scaffolds or refits projects from a profile. **Read `CONTEXT.md` before designing or building anything.** It is the product source of truth: artifacts, user journey, feature decisions and constraints.

## Status
- The repo is empty apart from `CONTEXT.md`, `LICENSE` (MIT) and `README.md`. No code, no build commands yet. Add them here once the Tauri scaffold exists.
- Stack decided: Tauri 2 + React + Vite + TypeScript, with a shared Rust core (`harneo-core`) for the app and the CLI.
- Modules and build order: `spec/capabilities.md`. Each module gets `spec/<module-id>.md` before any tickets (Addy's spec → plan → tasks → build). `spec/app-shell.md` also holds the project-wide conventions: commands, structure, code style, testing, boundaries.
- Cross-platform code, built and tested on Linux only. Distribution is source only (see `CONTEXT.md` §4–5).

## Quality bar
- **Read `CONSTRAINTS.md` before writing code. Do not weaken it to make a change pass.** Every rule blocks: `npm run check:task` must pass before a ticket is reported done, and `npm run check:full` before any merge to `main`.

## Specs and decisions
- **Specs live in `spec/`**, never at the repo root. When `/spec` (Addy's `spec-driven-development`) writes a new spec, save it as `spec/<module-id>.md`, not `SPEC.md`. `spec/capabilities.md` is the index of modules and specs.
- **ADRs only through Addy's framework:** write one with the `documentation-and-adrs` skill when a phase calls for it (e.g. a hard-to-reverse choice while specifying a module), in that skill's default location `docs/decisions/`. Never write ADRs by hand outside that flow.

## Tickets and workflow
- **Follow the `ticket-flow` skill** (`.claude/skills/ticket-flow/`) for all work. No change without a GitHub issue in `ibrahimkobeissy/harneo`.
- Board: GitHub Project 5 ("Harneo", https://github.com/users/ibrahimkobeissy/projects/5), with Status Backlog → Ready → In progress → In review → Done. Config: `.claude/ticket-flow.conf`. Move statuses with `.claude/skills/ticket-flow/scripts/board.sh`.
- Trunk-based: everything lands on `main`. No feature branches, no PRs. Subagents work in detached worktrees under `.claude/worktrees/`, and the orchestrator session validates, fixes, merges and pushes.
- Tickets use `.claude/skills/ticket-flow/references/ticket-template.md`, with dependencies written as `Blocked by #N`, one per line. Never delete a ticket; close it as "not planned" or as a duplicate.
- `tasks/plan.md` keeps only the ordered list of issue links, never a duplicate checklist. Do not write `tasks/todo.md` or use Addy's `/build auto`; the ticket-flow cycle replaces them.
- Cloud sessions have no `gh` and no board access: use the GitHub MCP tools for issues and record statuses as `status:*` labels, which the next local session's `board.sh sync` applies to the board.

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
