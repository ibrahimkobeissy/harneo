# Harneo

Harneo is a harness manager for Claude Code: a Tauri desktop app that keeps a library of AI Artifacts, composes profiles from it, and scaffolds or refits projects from a profile. **Read `CONTEXT.md` before designing or building anything.** It is the product source of truth: artifacts, user journey, feature decisions and constraints.

## Status
- The `app-shell` module exists: the Tauri app frame (`src-tauri/`, `src/`), `harneo-core` and the `harneo` CLI (`crates/`), the quality gates (`scripts/check-*.sh`, `CONSTRAINTS.md`) and the setup script (`scripts/setup.sh`). Library, profile and project features are not built yet. The repo also holds the product context, specs (`spec/`), the design system (`DESIGN.md`, `PRODUCT.md`) and the validated clickable prototype (`design/prototype/`, the visual reference for every UI ticket).
- Stack decided: Tauri 2 + React + Vite + TypeScript, with a shared Rust core (`harneo-core`) for the app and the CLI.
- Modules and build order: `spec/capabilities.md`. Each module gets `spec/<module-id>.md` before any tickets (Addy's spec → plan → tasks → build). `spec/app-shell.md` also holds the project-wide conventions: commands, structure, code style, testing, boundaries.
- Cross-platform code, built and tested on Linux only. Distribution is source only (see `CONTEXT.md` §4–5).

## Commands
Setup and per-OS prerequisites: `BUILDING.md` (start with `./scripts/setup.sh`, then `npm ci`).
- `npm run tauri dev`: run the app with hot reload.
- `npm run tauri build -- --no-bundle`: release binary, no installer.
- `cargo run -p harneo-cli -- --version`: run the CLI (`paths` prints the data folder).
- `npm run check:fast`: after each edit. `npm run check:task`: before a ticket is reported done. `npm run check:full`: before any merge to `main`.
- `npm test`, `npm run lint`, `npm run typecheck`, `npm run fix`: the building blocks. Shells may need `eval "$(mise env -s bash)"` to see the gate tools.

## Quality bar
- **Read `CONSTRAINTS.md` before writing code. Do not weaken it to make a change pass.** Every rule blocks: `npm run check:task` must pass before a ticket is reported done, and `npm run check:full` before any merge to `main`.

## Specs and decisions
- **Specs live in `spec/`**, never at the repo root. When `/spec` (Addy's `spec-driven-development`) writes a new spec, save it as `spec/<module-id>.md`, not `SPEC.md`. `spec/capabilities.md` is the index of modules and specs.
- **`WORKFLOW.md` is the user's log of the framework:** one line per step run (Addy's commands and Impeccable's), with what it produced. Add a line every time a step runs.
- **ADRs only through Addy's framework:** write one with the `documentation-and-adrs` skill when a phase calls for it (e.g. a hard-to-reverse choice while specifying a module), in that skill's default location `docs/decisions/`. Never write ADRs by hand outside that flow.

## Tickets and workflow
- **Follow the `ticket-flow` skill** (`.claude/skills/ticket-flow/`) for all work. No change without a GitHub issue in `ibrahimkobeissy/harneo`.
- Board: GitHub Project 5 ("Harneo", https://github.com/users/ibrahimkobeissy/projects/5), with Status Backlog → Ready → In progress → In review → Done. Config: `.claude/ticket-flow.conf`. Move statuses with `.claude/skills/ticket-flow/scripts/board.sh`.
- Trunk-based: everything lands on `main`. No feature branches, no PRs. Subagents work in detached worktrees under `.claude/worktrees/`, and the orchestrator session validates, fixes, merges and pushes.
- Tickets use `.claude/skills/ticket-flow/references/ticket-template.md`, with dependencies written as `Blocked by #N`, one per line. Never delete a ticket; close it as "not planned" or as a duplicate.
- `tasks/plan.md` keeps only the ordered list of issue links, never a duplicate checklist. Do not write `tasks/todo.md` or use Addy's `/build auto`; the ticket-flow cycle replaces them.
- **Releases:** a release is a `release_x.y.z` tag on `main` whose version matches `package.json`, `Cargo.toml` and `src-tauri/tauri.conf.json`. Pushing the tag runs the `check` workflow (`.github/workflows/check.yml`). Wait for that run: when it passes, publish a GitHub Release for the tag at once (`gh release create release_x.y.z --verify-tag --latest --title "Harneo x.y.z" --notes-file <notes>`), with notes shaped like [release_0.1.0](https://github.com/ibrahimkobeissy/harneo/releases/tag/release_0.1.0): what changed since the previous release, install (source only, link to the README's per-OS steps), platform status, privacy changes, thanks. When the run fails, publish nothing: fix it under a ticket, check it with a manual run (`gh workflow run check.yml --ref main`), move the tag only with the user's approval, and release once the run on the tag passes.
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
