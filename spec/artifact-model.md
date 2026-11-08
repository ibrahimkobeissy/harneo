# Spec: artifact-model

> Module `artifact-model` of the [capability map](capabilities.md). Status: **approved 2026-10-21.**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §2 (the 10 artifact types, what travels with each) and §5 (constraints).
> Project-wide conventions (commands, structure, code style, testing, boundaries) are in [`app-shell.md`](app-shell.md). This spec only records what this module adds or changes.

## Objective

Teach `harneo-core` what an AI Artifact is. Given a local folder, it finds every artifact of the 10 types, reads it, lists what it depends on, and checks it, without running anything inside it. Every later module builds on this: `library` stores what it finds, `audit` scores it, `bundles` groups it, `install-engine` writes it into projects.

**Users of this module:**
- the later modules, through a Rust API;
- the developer, through `harneo scan <path>`, to try real packs and projects before the library exists.

**Agent-neutral by design.** Everything Claude-specific (folder names, file formats, field names) sits behind an `AgentAdapter` trait, with one implementation, `ClaudeAdapter`. Adding Codex or Cursor later means a new adapter, not changes to the types (`CONTEXT.md` §1).

### Scope

- **Types** (`harneo-core/src/artifact/`): `ArtifactKind` (the 10 types), `Artifact`, `Reference`, `Diagnostic`, `ScanReport`, `ScanOptions`. Each derives `Serialize` and `ts_rs::TS`, ready for the library screens.
- **`AgentAdapter` trait:** `id()`, `scan(root, options) -> ScanReport`, and `install_path(artifact)`, the project-relative path from the "Installs to" column of `CONTEXT.md` §2.
- **Discovery.** `ClaudeAdapter::scan` recognises every common folder shape, and one folder can contain several:
  - **project**: `CLAUDE.md`, `.mcp.json` and `.claude/{rules,skills,commands,agents,output-styles,workflows}`, `.claude/settings.json`;
  - **`~/.claude`-like folder** or **pack repo**: the same children at the root (`skills/`, `agents/`, `commands/`, `rules/`, `output-styles/`, `workflows/`, `settings.json`, `CLAUDE.md`);
  - **plugin repo**: `.claude-plugin/plugin.json`. The folder becomes one Plugin artifact and the scan never descends into it, because plugins are kept whole; **amended 2026-11-01** ([`library.md`](library.md#unpacking-a-plugin), ADR-002): a folder in `ScanOptions.unpack` is reported as its pickable parts instead;
  - **marketplace repo**: `.claude-plugin/marketplace.json`. Each entry becomes one Plugin artifact. A remote source (a GitHub repo, a URL) is recorded as is and not fetched;
  - **loose skills**: any folder holding a `SKILL.md`, up to 4 levels deep, outside the shapes above.
  - **Amended 2026-10-25** by the layout resolver ([`library.md`](library.md#layout-resolver)): a plugin's files are its manifest and components, not its whole folder (**changed 2026-11-08 by ADR-005**: a plugin taken whole in its own folder keeps that whole folder; one at the repo root keeps its components, default component folders whole and the top-level entries its code names); a marketplace entry naming the imported repo merges with its plugin; `.agents/skills/` is a known pack folder; the scan also returns `unplaced` paths for Claude to propose a layout.
- **Parsing.**
  - **Markdown types** (CLAUDE.md, rules, skills, commands, subagents, output styles): YAML frontmatter plus body. The `name` and `description` fields are read into the artifact, and type-specific fields such as `tools`, `model`, `skills`, `mcpServers`, `paths` and `disable-model-invocation` are kept.
  - **JSON types**: Settings is one `settings.json` with its keys kept whole. MCP is one artifact per server in `.mcp.json`. A plugin's `plugin.json` gives its name, version and description.
  - **Workflows**: the `export const meta = { … }` object is read as text. The JavaScript is never run.
- **What travels with each artifact** (`files`): a skill's whole folder, the files a CLAUDE.md `@imports`, the scripts a Settings template's hooks and status line point to, and, for a plugin taken whole, its whole folder when it has its own, or its widened component scope when it is the repo (ADR-005, 2026-11-08; between 2026-10-25 and then, only its manifest and components).
- **References** (the input for `bundles`), from **structured fields only**:
  - a subagent's `skills` and `mcpServers`;
  - Settings' `outputStyle` and `enabledPlugins`, and its hook and status-line script paths;
  - CLAUDE.md `@imports`;
  - a command's or skill's `agent` field, if there is one.

  Each reference is marked **resolved** when the target was found in the same scan. Names mentioned in body text are left to the AI pass in `bundles`.
- **Checks** produce diagnostics; nothing fails the scan. Each diagnostic has a severity (`error` or `warning`), a stable `code`, a message and a file. Examples:
  - **Errors:**
    - YAML frontmatter or JSON that doesn't parse;
    - a subagent without `name` or `description`;
    - an MCP server with neither `command` nor `url`;
    - a plugin without a `name`;
    - a hook or status-line script that doesn't exist.
  - **Warnings:**
    - a skill without a `description`;
    - a skill whose `name` differs from its folder name;
    - a workflow without `meta`;
    - an `@import` that is missing, or points outside the scanned folder;
    - two artifacts of one type with the same name;
    - a skill and a command with the same name (the skill wins in Claude Code);
    - a **literal secret** in an MCP server's or Settings' `env` or `headers` instead of a `${VAR}` placeholder. The value is never printed, logged or stored. Only the key name is reported.
- **Digest**: a SHA-256 over each artifact's files (sorted relative path plus content), for `updates` and `drift` later.
- **CLI**: `harneo scan <path> [--json]`.
  - It prints each artifact's type, name, path, file count, references and diagnostics, then a summary line.
  - `--json` prints the `ScanReport`.
  - It exits 0 when the scan ran, whatever the diagnostics say, and 1 when it couldn't, for example when the path is missing or isn't a folder.

### Safety: scanned folders are untrusted

Imported packs come from strangers, so the scanner treats every folder as hostile:
- It **never executes** anything: no scripts, no workflow JavaScript, no `headersHelper`.
- It **never follows symlinks**. A symlink is listed as a file and reported with a warning, and its target is not read.
- It **reads only files that belong to an artifact.** It never reads `settings.local.json` or `CLAUDE.local.md`. It never reads files matching the secret patterns in `CLAUDE.md` (`.env*`, `*.pem`, key files, `*credentials*.json` and so on): they are listed by name inside an artifact with a warning, and their content stays out of the digest.
- **Limits** are set in `ScanOptions` with these defaults:
  - 1 MiB per parsed file (larger files are counted but not parsed, with a warning);
  - 100 MiB read per scan;
  - 4 levels of depth for loose skills;
  - `.git/`, `node_modules/` and `target/` skipped.

  The YAML parser runs with its size and alias limits on, so a crafted file can't exhaust memory.
- **Paths** in reports are always relative to the scanned root. A reference containing `..` that leaves the root is reported and never resolved.

**Out of scope:**
- importing, copying or storing anything (`library`);
- git URLs (`library` clones first, then calls `scan`);
- trust scores and security rules beyond the secret warning (`audit`);
- AI dependency detection (`bundles`);
- writing into projects (`install-engine`);
- any screen.

## Tech Stack

New dependencies for `harneo-core`. They are approved with this spec; adding any other still needs asking first.

| Crate | For |
|---|---|
| `serde` (+ derive), `serde_json` | Types over IPC and `--json`; Settings, `.mcp.json`, `plugin.json`, `marketplace.json` |
| `serde-saphyr` | YAML frontmatter. Maintained and pure Rust, with limits for untrusted input. `serde_yaml` is archived and `serde_yml` is flagged unsound (RUSTSEC-2025-0068) |
| `sha2` | Artifact digest |
| `ts-rs` | TS bindings for the types (already used by `harneo-app`) |
| `tempfile` (dev only) | Integration tests that build folders on the fly |

Versions are the latest stable releases when the first ticket starts, checked on crates.io and pinned by `Cargo.lock`.

## Commands

The project commands from `app-shell.md`, plus:

```bash
cargo run -p harneo-cli -- scan <path>          # human-readable scan report
cargo run -p harneo-cli -- scan <path> --json   # ScanReport as JSON
cargo test -p harneo-core artifact              # this module's tests
```

## Project Structure

```
crates/harneo-core/src/artifact/
  mod.rs          → public types: ArtifactKind, Artifact, Reference, Diagnostic, ScanReport, ScanOptions
  adapter.rs      → the AgentAdapter trait
  frontmatter.rs  → YAML frontmatter split + parse (agent-neutral)
  walk.rs         → safe folder walk: no symlinks, limits, skip lists, secret-file patterns
  digest.rs       → SHA-256 over an artifact's files
  claude/         → ClaudeAdapter
    mod.rs        → scan(): layout detection, then one parser per type
    layout.rs     → project / ~/.claude / pack / plugin / marketplace / loose-skill detection
    kinds/        → one file per type: claude_md.rs, rule.rs, skill.rs, command.rs, agent.rs,
                    settings.rs, mcp.rs, output_style.rs, workflow.rs, plugin.rs
    checks.rs     → cross-artifact checks: duplicate names, reference resolution
crates/harneo-core/tests/
  artifact_scan.rs → integration tests against fixtures
  fixtures/artifact/ → project/, claude-home/, pack/, plugin/, marketplace/, hostile/
crates/harneo-cli/src/scan.rs → `harneo scan` argument parsing and printing
src/bindings/     → generated TS for the artifact types (committed)
```

## Code Style

```rust
// crates/harneo-core/src/artifact/adapter.rs
/// One coding agent's knowledge of where its artifacts live and what they look like.
/// The rest of Harneo only sees `Artifact`s, so another agent is a new adapter, not a rewrite.
pub trait AgentAdapter {
    fn id(&self) -> &'static str;
    fn scan(&self, root: &Path, options: &ScanOptions) -> Result<ScanReport>;
    /// Where this artifact goes in a project, relative to the project root.
    fn install_path(&self, artifact: &Artifact) -> PathBuf;
}
```

```rust
// crates/harneo-core/src/artifact/claude/kinds/agent.rs
pub(crate) fn check(meta: &AgentMeta, file: &Path, out: &mut Vec<Diagnostic>) {
    if meta.description.is_none() {
        out.push(Diagnostic::error("agent.missing-description", file, "a subagent needs a `description`; Claude uses it to decide when to delegate"));
    }
}
```

Conventions added by this module:
- **Diagnostic codes** are `<kind>.<kebab-problem>`, for example `skill.missing-description`. They are stable once released: `audit` and the UI may key on them.
- **Parsers are pure.** They take file contents plus a relative path and return an artifact and diagnostics. Only `walk.rs` touches the file system, so every parser is unit-tested without files.
- **Never `panic` on input.** Malformed input becomes a diagnostic.

## Testing Strategy

The levels follow `app-shell.md`. This module adds:
- **Unit tests, one per check:** every diagnostic code has a test that triggers it and a test that doesn't.
- **Fixture integration tests** (`tests/artifact_scan.rs`):
  - each folder shape gives the expected artifacts (kind, name, files, references, diagnostics);
  - the `install_path` of each type matches `CONTEXT.md` §2.
- **Hostile fixture** (`fixtures/hostile/`):
  - a symlink pointing outside the root;
  - `../` references;
  - a `.env` and a key-named file inside a skill;
  - a file over the size limit;
  - a YAML alias bomb;
  - invalid UTF-8;
  - deep nesting.

  The scan finishes, reads none of the forbidden content and reports each problem. The `.env` and key-named files are generated at test time, so no secret-pattern file is committed and gitleaks stays clean.
- **Secret warning:** a literal token in a fixture's MCP `env` gives `mcp.literal-secret`, and the test asserts the value appears nowhere in the report or its JSON.
- **CLI** (`assert_cmd`): `harneo scan` on a fixture prints the summary and exits 0; `--json` gives valid JSON; a missing path exits 1.
- **Coverage:** changed lines 80% or more, and the project ratchet must not fall (`CONSTRAINTS.md`).

## Boundaries

- **Always:**
  - treat scanned folders as untrusted (see Safety);
  - keep Claude-specific knowledge inside `artifact/claude/`;
  - give each new check a stable code and a test;
  - report paths relative to the scanned root.
- **Ask first:**
  - adding an artifact type or a field to the public types after the first release of `library`;
  - adding a dependency beyond the table above;
  - reading any file that doesn't belong to an artifact.
- **Never:**
  - execute or evaluate anything found in a scanned folder;
  - follow symlinks;
  - print, log or store a secret value;
  - read `settings.local.json`, `CLAUDE.local.md` or secret-pattern files;
  - write anywhere: this module only reads.

## Success Criteria

1. `harneo scan` on each fixture shape (project, `~/.claude`-like, pack, plugin, marketplace) lists exactly the expected artifacts, of all 10 types across the set, each with its files, references and diagnostics.
2. Every check listed in Scope has a diagnostic code, a test that triggers it and a test that doesn't.
3. The hostile fixture scans to completion. No symlink target, secret-pattern file or `..` path is read, and each one is reported.
4. A literal secret in `.mcp.json` or `settings.json` gives a warning that names the key, and its value appears nowhere in the output, human-readable or `--json`.
5. `install_path` gives the `CONTEXT.md` §2 location for every type.
6. The same folder always gives the same report and digests. Changing one byte of a skill's script changes that skill's digest only.
7. The types are exported to `src/bindings/` and compile in the frontend.
8. Manual check by the user: `harneo scan` on real sources (their `~/.claude`, an Addy `agent-skills` clone, the Impeccable plugin) finishes in under 2 seconds and lists what they expect. Anything wrong becomes a fixture and a fix.
9. `npm run check:full` passes and the coverage ratchet holds.

## Decisions (2026-10-21)

1. **All common folder shapes are recognised:** project, `~/.claude`-like, pack, plugin, marketplace and loose skills.
2. **References come from structured fields only.** Free-text mentions are the AI pass in `bundles`.
3. **The module surface is the CLI `harneo scan <path>` (`--json`).** It has no screen. The library screens come with `library`.
4. **A literal secret is a warning, not an error.** The scan reports it by key name and never shows the value.

## Open Questions

1. **Literal secrets vs "No secrets stored"** (`CONTEXT.md` §5). With decision 4 the scan only warns. The `library` spec has to decide what happens at import. Options: replace the value with a `${VAR}` placeholder automatically, ask the user to do it before importing, or store it as is. Storing it as is would conflict with §5.
