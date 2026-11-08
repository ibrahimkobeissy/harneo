# ADR-005: A plugin taken whole keeps its whole folder

## Status
Accepted. Supersedes the file scope of a whole plugin in ADR-001 ("Plugins list, copy and audit only their components"). ADR-001's three tiers, and the component scope that decides which parts can be unpacked (ADR-002, ADR-004), still hold.

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
A plugin taken whole keeps **every file in its folder**, like Claude Code's install, with these limits:
- **Nested plugins stay out.** A sub-folder that is itself a plugin or a marketplace entry of the same source is its own candidate, not a file of the outer plugin. This covers a marketplace at the repo root whose plugins live under `plugins/`.
- **Skipped folders stay out:** the ones the scanner already skips everywhere (`ScanOptions::skip_dirs`: `.git`, `node_modules`, `target`, `.trash`).
- **The existing per-file rules apply unchanged:** the 1 MiB large-file limit (copied only on request, listed as not audited), symlinks, and secret replacement.
- Every kept file is **audited**.

The component scope (`plugin_scope.rs`) is still computed: it decides the pickable parts when the plugin is unpacked, the "stays whole" reasons, and what the Library lists as the plugin's components. Files outside it are shown as the plugin's other files.

Plugins already in the library get the missing files at their next update check, which reports them as changed files of the plugin.

## Alternatives Considered

### Keep the allowlist and follow paths written in scripts
- Pros: smaller copies, no repo docs in the audit.
- Cons: guesswork over arbitrary code (Python, shell, JS); misses paths built at run time; every miss is a broken plugin that looks fine in Harneo.
- Rejected: Harneo would still differ from what Claude Code installs.

### Keep the allowlist and add the plugin's `templates/` and `hooks/` folders whole
- Pros: fixes this plugin with a small change.
- Cons: the next plugin keeps its data in `assets/`, `prompts/` or `lib/`.
- Rejected: a convention list cannot cover what authors do.

### Take the folder only when the allowlist misses a referenced file
- Pros: copies stay small in the common case.
- Cons: needs the same unreliable reference detection to know when to switch.
- Rejected.

## Consequences
- A whole plugin in the library matches what Claude Code installs; its hooks and skills find their files.
- A plugin at a repo root takes the repo's other files (docs, tests, CI), as Claude Code would. `mattpocock/skills` (a marketplace pointing at `./`) is stored with its whole repo again, about 191 files: exactly what Claude Code installs from it. The audit sees more files and may report more findings. Nested plugins and skipped folders are the only cuts.
- `spec/library.md` changes: "never its whole repo" becomes "its whole folder, minus nested plugins and skipped folders".
- Update checks report the newly kept files once for plugins imported before this change.
- Unpacking is unchanged: parts are the components, and a plugin's other files are never parts.
