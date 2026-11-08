# ADR-005: A plugin taken whole keeps its whole folder, unless it is the repo itself

## Status
Accepted (hybrid rule, user decision 2026-11-08). Supersedes the file scope of a whole plugin in its own folder in ADR-001 ("Plugins list, copy and audit only their components"). ADR-001's three tiers, and the component scope that decides which parts can be unpacked (ADR-002, ADR-004), still hold.

## Date
2026-11-08

## Context
ADR-001 made the plugin manifest authoritative and kept, for a plugin taken whole, only an allowlist of files (`plugin_scope.rs`): the manifest, the declared or default component folders and files, files named through `${CLAUDE_PLUGIN_ROOT}/…`, and the root `README*` and `LICENSE*`. The aim was to stop a repo being stored as one huge plugin (`mattpocock/skills`, 191 files) and to keep repo docs and tests out of the audit.

Importing `ibrahimkobeissy/claude-software-factory-basic` (2026-11-08) showed the cost. Its `security-guardrails` plugin was offered whole with 10 of its 14 files. Dropped:
- `hooks/rules.md`, read by the `SessionStart` hook script (`hooks/session_start.py`, next to it);
- `templates/settings.json` and `templates/gitleaks.toml`, read by that hook and by the `setup` skill's script, which merges them into the project's `.claude/settings.json` and `.gitleaks.toml`;
- `.claude-plugin/icon.png`.

Scripts reach these files through paths built at run time (relative to their own location), not through `${CLAUDE_PLUGIN_ROOT}` in a manifest, so no allowlist can see them. The library copy was broken: its session hook and its setup could not work, and the audit never saw the settings template that setup applies.

Claude Code installs a plugin by copying its **whole folder** into its plugin cache; only files outside the folder are unavailable. A whole plugin in Harneo should be what Claude Code would install.

A whole plugin still installs in projects from its marketplace (`spec/install-engine.md` 4b), so projects got the full plugin from upstream; the library copy is what is audited, shown, diffed by updates, and kept for durability (ADR-004).

## Decision
The files of a plugin taken whole depend on where the plugin sits (user decision 2026-11-08, after a first draft that took the whole folder everywhere brought back the `mattpocock/skills` problem):

- **A plugin in its own folder** (`plugins/security-guardrails/`, or a plugin Claude Code installed in its cache) keeps **every file of that folder**, like Claude Code's install. A plugin's own folder holds only the plugin, so this is exact and needs no guessing.
- **A plugin at the scan root** (the repo is the plugin: a marketplace entry `"source": "./"`, or a `plugin.json` at the root) keeps:
  - its component scope (ADR-001's allowlist: manifest, declared and default components, `${CLAUDE_PLUGIN_ROOT}` references, root `README*` and `LICENSE*`);
  - Claude Code's **default component folders kept whole** (`.claude-plugin`, `skills`, `commands`, `agents`, `hooks`, `output-styles`, `workflows`, `themes`, `monitors`, `bin`) when the scope reaches them, so `hooks/rules.md` or `.claude-plugin/icon.png` come along;
  - the **top-level entries its own code files name** as a path segment (between quotes or slashes, in `.py`, `.sh`, `.js`, `.ts`, `.json`, `.toml`… files in the scope), so `templates/` comes along when a hook script builds `ROOT / "templates"`. Prose (Markdown) does not count.

  The repo's plumbing (`docs/`, `.github/`, `.changeset/`, `package.json`…) stays out.

In both cases:
- **Nested plugins stay out.** A sub-folder that is itself a plugin or a marketplace entry of the same source is its own candidate.
- **Skipped folders stay out:** the ones the scanner already skips everywhere (`ScanOptions::skip_dirs`: `.git`, `node_modules`, `target`, `.trash`).
- **The existing per-file rules apply unchanged:** the 1 MiB large-file limit, symlinks, secret replacement. Every kept file is audited.

The component scope (`plugin_scope.rs`) still decides the pickable parts when the plugin is unpacked and the "stays whole" reasons.

Checked on the real repos: `security-guardrails` keeps its 14 files (it had 10); `mattpocock/skills` keeps 118 (its skills, manifest, README, LICENSE), not its 184.

Plugins already in the library get the missing files at their next update check, which reports them as changed files of the plugin.

## Alternatives Considered

### The whole folder everywhere (this ADR's first draft)
- Pros: one exact rule, identical to Claude Code's install.
- Cons: a plugin that is the repo takes its docs, CI and changesets: `mattpocock/skills` back to 184 files, the problem ADR-001 fixed.
- Rejected by the user.

### The widened allowlist everywhere
- Pros: one rule; works on both real repos.
- Cons: even a self-contained plugin would depend on the "named by code" guess; a script that builds a folder name in pieces would lose files.
- Rejected: guess only where the exact rule can't be used.

### Keep the allowlist and follow every path written in scripts
- Pros: smaller copies.
- Cons: guesswork over arbitrary code, including file names built at run time; every miss is a broken plugin that looks fine in Harneo.
- Rejected.

## Consequences
- A plugin in its own folder matches what Claude Code installs; its hooks and skills find their files.
- A plugin at the repo root can still miss a data folder its code reaches in a way the name search doesn't see (a name built in pieces, a path read from a config file outside the scope). The Library shows the plugin's files, and the user can check them.
- `spec/library.md` and `spec/artifact-model.md` describe the hybrid rule.
- Update checks report the newly kept files once for plugins imported before this change.
- Unpacking is unchanged: parts are the components, and a plugin's other files are never parts.
