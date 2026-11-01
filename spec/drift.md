# Spec: drift

> Module `drift` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stage 6.1–6.2 (live rescan, drift check: re-apply or promote). Visual reference: `design/prototype/projects.js:7-12,40-49,73-80` (status pill, *Drift* tab, re-apply, promote).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`install-engine.md`](install-engine.md) (registry, manifest, plans, apply) and [`updates.md`](updates.md) (library copy backups).

## Objective

Keep every registered project honest. Harneo notices, live, when a managed file in a project no longer matches what it wrote ("3 files in *my-api* no longer match *Heavy v4*"), shows the change as a diff, and offers two ways out: **re-apply** the profile's version (the project's version backed up first) or **promote** the project's change into the library so every project can get it. It also tells a project when the library has newer copies of what it runs.

**User:** the builder who tweaked an agent inside one project. Success: the project's card turns amber, the Drift tab shows `+6 −2` on `code-reviewer.md`, and *Promote to library* makes that the library copy, after which the other projects on the same profile show "Library has newer copies".

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Drift is measured against the manifest** (`installed_files`: path + digest written by Harneo): *edited in project*, *deleted in project*. New artifacts in the project's harness that Harneo didn't write are *added in project* (shown separately, promote = import).
2. **Library newer** is a separate signal: an installed file whose artifact's library copy now renders differently (after an update or a promote). It is shown as "Library has newer copies of N files" with **Re-apply**, not as drift.
3. **Re-apply** writes what the project's pinned profile version renders **now** (current library copies), through `install-engine`'s plan and apply (backup first).
4. **Promote** replaces the library copy of that artifact with the project's file (library backup first, as `updates` does; re-audit; `copy_digest` updated so it is not a "local edit"); the profile is unchanged; the manifest of this project is updated so it is in sync. Deleted files can't be promoted. Generated files (`HARNESS.md`, `.mcp.json`, a `settings.json` made only of plugins) can only be re-applied. **Amended 2026-11-01 (#196):** a Settings template with the profile's plugins merged in can be promoted: the `enabledPlugins` and `extraKnownMarketplaces` entries the install added go back to the template's state (removed, or the template's own value), and everything else, including plugin entries added by hand in the project, becomes the library copy. Promote is refused when the project changed or removed an entry the profile sets, since only the profile can say that.
5. **A missing project** (its folder is gone) can be removed from Harneo without a plan: *Remove from Harneo* on the missing callout (and the Remove action) marks it removed like a clean removal, touches nothing on disk, and keeps its backup sets. Refused while the folder exists (`forget_missing_project`).
6. **Live rescan** uses the `notify` crate on the library store and each registered project's harness paths (`.claude/`, `CLAUDE.md`, `.mcp.json`, `HARNESS.md`), debounced 500 ms; it only triggers re-checks and UI refreshes, never writes. A project folder that disappears is shown as *Missing* and can be removed from the registry.

### Scope

**Core (`harneo-core/src/drift/`)**
- `check_project(id) -> DriftReport { status: InSync | Drift | Missing, files: Vec<DriftFile{ path, change: Edited{ plus, minus } | Deleted | Added, artifact }>, library_newer: Vec<path>, profile_newer: Option<u32> }` (line counts from `similar`).
- `check_all() -> Vec<(ProjectId, DriftSummary)>`; `drift_diff(project, path) -> Vec<Hunk>` (profile version → project version).
- `plan_reapply(project, paths: All | Some(Vec<path>)) -> Plan` (install-engine's `Plan`, applied with `apply`).
- `promote(project, path) -> PromoteReport` and `promote_added(project, path)` → returns a library `Source::Local` preview of the project with that artifact preselected.
- **Watcher (`drift/watch.rs`):** `Watcher::start(paths, on_change)` over `notify` with debounce; the paths list is rebuilt when the registry changes. Core exposes the watcher; `src-tauri` owns its lifetime and turns events into a Tauri event `harneo://changed { scope: "library" | { project: id } }`.

**App**
- **IPC:** `drift_check`, `drift_check_all`, `drift_diff`, `drift_reapply_plan`, `drift_promote`, `drift_promote_added`; the event `harneo://changed`, which the frontend turns into query invalidation (library list, detail, project, projects list).
- **Projects list:** status pill *Drift · N files* (amber), *Profile vN available* (blue), *Library has newer copies* (blue), *Missing* (red), *In sync* (green); sidebar Projects shows the drifted count.
- **Project detail:** *Drift* tab (default when drifted): warn callout with **Re-apply all**, a tree of drifted files marked *deleted* or *+N −M*, the diff viewer with **Promote to library** (disabled for deleted and generated files, with a tooltip saying why) and **Re-apply profile version**; *Added in project* group with **Import to library**; the library-newer callout with **Re-apply**; confirms and toasts per the prototype. *No drift* state.
- A small "Watching N projects" status in Settings, and the watcher can be paused there.

**Out of scope:** editing files in Harneo; three-way merges (never); drift for unregistered folders.

## Tech Stack

New dependency (pre-approved with this spec): `notify` (core) for the watcher, with its debouncer (`notify-debouncer-full` or the version's built-in equivalent); latest stable.

## Commands

```bash
cargo test -p harneo-core drift
npx vitest run src/features/projects
```

## Project Structure

```
crates/harneo-core/src/drift/
  mod.rs     → DriftReport, check, diff, re-apply plan, promote
  watch.rs   → notify watcher with debounce
  *_tests.rs
src-tauri/src/watch.rs → watcher lifetime, Tauri events
src/features/projects/DriftTab.tsx, src/lib/useChanged.ts
```

## Code Style

```rust
/// A managed file drifts when its bytes no longer hash to what Harneo wrote.
fn classify(file: &InstalledFile, root: &Path) -> Result<Option<DriftChange>> {
    let path = root.join(&file.path);
    if !path.exists() { return Ok(Some(DriftChange::Deleted)); }
    let now = digest_file(&path)?;
    Ok((now != file.digest).then(|| DriftChange::edited(&file.written, &read(&path)?)))
}
```

## Testing Strategy

- Edited, deleted and added files detected against a manifest; line counts; missing project.
- Library newer after an update and after a promote elsewhere; profile newer.
- Re-apply backs up the project's version and restores sync; promote updates the library copy (backup, re-audit) and this project's manifest; generated and deleted files refused.
- Watcher: a write in a temp project fires one debounced event; writes outside the watched paths don't.
- Frontend (mockIPC + a mocked event): pills, Drift tab actions, invalidation on `harneo://changed`.

## Boundaries

- **Always:** backup before re-apply and before promote; the watcher only reads.
- **Ask first:** watching anything beyond the library store and registered projects' harness paths.
- **Never:** write to a project without the user's action; merge.

## Success Criteria

1. A managed file edited or deleted in a project shows as drift within a second, with a diff.
2. Re-apply restores the profile's version with a backup; promote makes the project's version the library copy.
3. Projects on the same profile then show *Library has newer copies*.
4. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: "library newer" shown apart from drift; promote keeping the profile version unchanged.
