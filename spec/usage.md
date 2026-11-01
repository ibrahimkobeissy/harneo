# Spec: usage

> Module `usage` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stage 2.2 (usage on each row, read from `~/.claude/projects/` so past sessions count), Stage 7 (cleanup suggestions, prune and push), §5 (reading Claude's session logs is the one allowed read outside the library and projects), §7 (Loadout: show "not tracked" rather than a misleading zero). Visual reference: `design/prototype/library.js:101-131,305-309` (health bar, sessions chart, card footer, Sessions column, drawer *Usage* and *Used in*), `core.js:43-58` (status model), `projects.js:111-150` (Cleanup screen, Settings rows).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`library.md`](library.md), [`profiles.md`](profiles.md) and [`install-engine.md`](install-engine.md) (prune pushes through switch).

## Objective

Show which artifacts the user actually uses, from Claude's own session logs, so past sessions count with no setup. Each usage-tracked artifact gets sessions in the last 8 weeks, last used, and an 8-week activity graph; the library gets a health bar and a sessions chart; and a **Cleanup** screen suggests what to prune (risky, outdated, unused, repeated) with one-click actions that go through profiles and projects safely.

**User:** the builder whose harness has grown. Success: Cleanup says "`webperf` unused for 9 weeks, in Heavy and Light"; *Remove from profiles* saves new versions and switches the two projects with backups.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Read-only, names only.** Harneo reads `~/.claude/projects/**/*.jsonl` line by line and keeps only: session id, day, project path (`cwd`), and the name of each skill, subagent, command, MCP server and plugin used. **No message text, prompt, tool input other than the name, or output is ever stored, logged or sent.**
2. **What counts as a use** (per session, so ten calls in one session are one session):
   - skill: an assistant `tool_use` named `Skill` (its `skill` input; `plugin:skill` also counts for the plugin);
   - subagent: a `tool_use` named `Task` or `Agent` (its `subagent_type`);
   - command: a user message holding `<command-name>/name</command-name>`;
   - MCP server: a `tool_use` named `mcp__<server>__…`;
   - plugin: any of the above namespaced `plugin:` / `plugin_name:`.
   Rules, Settings, CLAUDE.md, output styles and workflows are **not tracked** and say so ("Not tracked"), showing "In N projects" instead. The parser tolerates unknown lines and fields (the log format is Claude's, not ours) and counts parse failures in a diagnostic.
3. **Matching** to library artifacts is by kind and name at query time, so a re-import or a duplicate shares the history; duplicates are marked.
4. **Status** (prototype `core.js:43-58`), with this precedence: *findings* (trust < 60) → *outdated* → *unused* (tracked, 0 sessions in the unused window, imported longer ago than that window) → *not used yet* (tracked, 0 sessions, imported recently) → *current*. The "unused" window is a Setting (default 4 weeks; the user's choice, 2026-11-01).
5. **Index** in its own database, `<data dir>/usage/usage.db`, incremental by file size and modification time (a file that shrank is re-read); the first index runs in the background with progress. Refreshed at app start, every 10 minutes while the app is open, and on demand.

### Scope

**Core (`harneo-core/src/usage/`)**
- **Store:** `log_files(path PK, size, mtime, offset, parsed_lines, failed_lines)`, `uses(session_id, day, project_path, kind, name, PRIMARY KEY(session_id, kind, name, day))`.
- **Indexer:** `index(logs_root, progress) -> IndexReport { files, new_uses, failed_lines }` streaming each file (`BufRead`, lines over 4 MiB skipped), never following symlinks, ignoring non-`.jsonl` files.
- **Queries:** `usage_of(kind, name) -> Usage { sessions_8w, weekly: [u32; 8], last_used: Option<day>, projects: Vec<path> }`; `usage_summary() -> { this_week, change_pct, weekly: [u32; 8] }`; `LibraryItem` gains `usage: Option<UsageBrief{ sessions, last_used }>` (None = not tracked) and `status`; `SortKey::Sessions`; `Filter` gains `statuses`; facets count statuses; smart collection rules gain a Status row (the prototype's *To clean up* = unused or outdated).
- **Cleanup suggestions:** `cleanup() -> Cleanup { findings, outdated, unused: Vec<{ artifact, last_used, profiles }>, repeated: Vec<{ artifact, profiles }> /* in ≥ 3 profiles */, duplicates: Vec<{ name, kind, ids }> }`.
- **Prune:** `plan_prune(artifact) -> PrunePlan { profiles: Vec<(id, new_version)>, projects: Vec<(id, Plan)> }` then `apply_prune`: removes the artifact from each profile (new versions) and switches each project on those profiles to the new version through `install-engine` (backups as always). The artifact stays in the library.

**App**
- **IPC:** `usage_index` (async, progress events), `usage_of`, `usage_summary`, `cleanup_list`, `prune_plan`, `prune_apply` (async); Settings `usage.unusedWeeks`, `usage.logsPath` (display only).
- **Library:** card footer "N sessions" / "unused · last X" (card greyed) / "not used yet" / "In N projects" for untracked kinds; list *Sessions* column (tooltip "Sessions in the last 8 weeks", "—" untracked) and *Status* column; sort *Most used*; **Status** filter (Findings, Outdated, Unused, Not used yet, Current); **Library insights**: *Library health* stacked bar with legend buttons that filter, "Review N issues" → Cleanup, and *Sessions using library artifacts* (8 bars, "N this week · ±X%", no-data state with the prototype copy).
- **Detail panel:** *Usage* section ("N sessions in 8 weeks · last X", 8 bars, projects) for tracked kinds, "Not tracked: Claude's logs don't record when X is used." otherwise; *Used in* gains projects.
- **Cleanup screen** (`/cleanup`, prototype `projects.js:111-139`): groups *Security findings* (Remove), *Outdated* (Update, Update all — from `updates`), *Unused for N+ weeks, in every project* (Remove from profiles / Remove; first 12, "See all N in the library"), *Repeated across profiles* (with the note), *Duplicate names*; the prune confirm with the prototype copy; empty state.
- **Settings screen:** *Session logs (read only)* row with the path, last index time, **Re-index**; *Data folder* row; the unused window.

**Out of scope:** usage of artifacts outside the library (not shown); per-hook or per-rule usage (not in the logs); sending usage anywhere; editing or deleting Claude's logs (never).

## Tech Stack

No new dependency. The logs path comes from `paths::claude_home()`.

## Commands

```bash
cargo test -p harneo-core usage
npx vitest run src/features/library src/features/cleanup src/features/settings
```

## Project Structure

```
crates/harneo-core/src/usage/
  mod.rs       → store, queries, status
  parse.rs     → one JSONL line → uses (names only)
  index.rs     → incremental indexer
  cleanup.rs   → suggestions, prune plan
  *_tests.rs   (synthetic logs only)
src/features/cleanup/, src/features/library/{Insights,UsageSection}.tsx, src/features/settings/
```

## Code Style

```rust
/// Pulls the names of what a log line used. Everything else in the line is dropped here,
/// so no message text ever reaches the index.
pub(crate) fn uses_in(line: &serde_json::Value) -> Vec<Use> { … }
```

## Testing Strategy

- **Synthetic logs only,** written by the tests in the shape described in Decision 2 (and unknown shapes, broken JSON, huge lines). **No test, fixture or agent reads the real `~/.claude/projects/`** — it holds the user's conversations.
- Parser: each use type, plugin namespacing, sessions counted once, unknown lines tolerated; a test asserts that a distinctive message text in the fixture never appears in `usage.db`.
- Indexer: incremental append, shrink → re-read, symlink ignored, progress.
- Status precedence and the unused window; cleanup groups; prune plan through profiles and a registered project (backup made).
- Frontend (mockIPC): footer states, columns, sort, Status filter, insights, detail Usage, Cleanup groups and confirms, Settings rows.
- Performance: indexing 100 MB of synthetic logs under 10 s; a library page with usage under 100 ms.

## Boundaries

- **Always:** read-only on the logs; names only; "Not tracked" over a false zero.
- **Ask first:** reading any other file under `~/.claude`.
- **Never:** store or send message text; modify Claude's logs; let an agent read the user's real logs.

## Success Criteria

1. Sessions, last used and the 8-week graph appear for tracked kinds; untracked kinds say so.
2. Status, the Status filter, *Most used* and the insights panels work.
3. Cleanup lists findings, outdated, unused, repeated and duplicates; *Remove from profiles* saves new profile versions and switches projects with backups.
4. No message text from the logs is ever stored (test).
5. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: what counts as a use (Decision 2); the 8-week window; refresh every 10 minutes.
