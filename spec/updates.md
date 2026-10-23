# Spec: updates

> Module `updates` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stage 6.3–6.4 (outdated flag, update, protect local edits), §7 (HarnessKit's flaws: silent overwrites; an update check that ignores branch and subfolder). Visual reference: `design/prototype/library.js:294,313,361-375` (drawer callout, provenance, update and local-edit dialogs), `core.js:53,61` (badge, source line), `projects.js:121,133-139` (Cleanup *Outdated* group, *Update all*).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`library.md`](library.md) (sources, provenance, atomic copies, `git.rs`) and [`audit.md`](audit.md) (re-audit after update).

## Objective

Tell the user when an artifact changed **at its source** since they imported it, and let them update the library copy in one click, **never overwriting their own edits silently**. The check is per artifact (its own files, at its own branch and subfolder), not "the repo moved". Projects get updated copies only when they re-apply or switch their profile.

**User:** the builder who imported packs months ago. Success: "`/ship` · Update 3.4.1" appears on its card; the user edited `release` locally, so its update shows *Yours* vs *Upstream* side by side and they choose; nothing else changes.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Outdated means the artifact's own content changed upstream:** the upstream scan digest of that artifact (same path in the source) differs from the digest recorded at import. A repo commit that didn't touch it is not an update.
2. **Git checks are cheap first:** `git ls-remote <url> <branch or HEAD>`; only when the remote commit differs from the recorded one is the repo shallow-cloned and rescanned. Local sources are rescanned in place (read-only).
3. **Local edits** = the library copy's current content differs from what was stored at import (a `copy_digest` over the stored files, recorded at import and after each update; rows imported before this module get it on first check and count as unedited).
4. **Keep mine** marks that upstream digest as *skipped*: no flag until upstream changes again. **Take upstream** backs up the edited copy to `<data dir>/library/.backups/<id>/<UTC timestamp>/` first.
5. **Automatic check** at app start when the last one is older than 24 h (Settings toggle, default on), in the background; plus **Check for updates** on demand. Network access is only through `git`.
6. **Versions shown** are the plugin or package `version` when the artifact has one (`plugin.json`, `package.json`, frontmatter `version`), else short commits (`5e0b7aa → 9c1d2e3`), else "changed".

### Scope

**Core (`harneo-core/src/updates/`)**
- **Schema migration v4:** `artifacts.copy_digest TEXT`, `artifacts.updated_at INTEGER`; `sources.checked_at INTEGER`, `sources.remote_commit TEXT`; `upstream(artifact_id PK REFERENCES artifacts ON DELETE CASCADE, state CHECK outdated|removed|skipped, upstream_digest, upstream_commit, from_version, to_version, checked_at)`.
- **Check:** `check_updates(scope: All | Source(id) | Artifact(id)) -> CheckReport { sources: Vec<SourceCheck{ id, result: Current | Changed{ outdated, removed, new_upstream } | Unreachable(reason) | Missing }> }`. One source failing (offline, auth, missing folder) never stops the others. The clone is kept in the data dir's temp area for the session (reused by *Update*), swept like other clones.
- **Status per artifact:** `update_status(id) -> Option<UpdateStatus{ state, from_version, to_version, local_edits: bool }>`; `LibraryItem` gains `update: Option<UpdateBadge{ to_version, local_edits }>`; `Filter` gains `outdated: bool`; facets count outdated.
- **Diff:** `update_diff(id) -> Vec<FileDiff{ path, status: Added|Removed|Changed, hunks }>` between the library copy and upstream (`similar`, pre-approved in `install-engine.md`).
- **Update:** `update(id, choice: TakeUpstream | KeepMine) -> UpdateReport`. Take upstream: backup when edited, then the same per-type copy as import (secret replacement, server.json for MCP, plugins whole) into a temp folder and an atomic swap; provenance digest, commit, `updated_at`, `copy_digest` updated; re-audit; reference links (`bundles`) refreshed; the upstream row cleared. Keep mine: state → skipped.
- **Update all:** `update_all() -> Vec<UpdateReport>` updates every outdated artifact **without** local edits and returns the skipped ones.
- **Removed upstream:** shown as "Removed at source", never deleted automatically; the user can remove it from the library.
- Local-edit detection reads the store only; it never writes to a project.

**App**
- **IPC:** `updates_check` (async, reports per source), `update_status`, `update_diff`, `update_apply` (async), `update_all` (async); Settings `updates.autoCheck`, `updates.lastCheck`.
- **Library:** card badge *Update X* (warn) and source line "label · from → to"; list Status column shows it; **Status: Outdated** filter; drawer callout "Update available: from → to" with "Upstream changed since you imported it." or the local-edit text, **Update** button; provenance "Version X [Y available]"; toolbar **Check for updates** with a last-checked time and the per-source errors in a dismissible callout.
- **Update dialog** (prototype `library.js:361-365`): changes list from the diff (+ / ~ / − per file, expandable hunks), **Update library copy**.
- **Local-edit dialog** (prototype `library.js:366-375`): *Yours (edited locally)* vs *Upstream X* side by side per file, **Keep mine** / **Take upstream** (with the backup path), Confirm.
- **Update all** (from the library toolbar and the Cleanup *Outdated* group when `usage` lands): confirm with the skipped-with-edits callout.

**Out of scope:** pushing updates into projects (re-apply / switch in `install-engine`, flagged by `drift`); merging upstream with local edits (never); vulnerability databases.

## Tech Stack

No new dependency of its own: `similar` (line diffs) is pre-approved in `install-engine.md`; whichever ticket needs it first adds it.

## Commands

```bash
cargo test -p harneo-core updates
npx vitest run src/features/library
```

## Project Structure

```
crates/harneo-core/src/updates/
  mod.rs      → check, status, update, update_all
  remote.rs   → ls-remote and the reused clone (through git.rs)
  diff.rs     → per-file diffs
  *_tests.rs
src/features/library/UpdateCallout.tsx, UpdateDialog.tsx, LocalEditsDialog.tsx
```

## Code Style

```rust
/// An artifact is outdated only when its own files changed upstream: the repo moving is not enough.
fn classify(stored: &StoredArtifact, upstream: Option<&Artifact>) -> Option<UpstreamState> {
    match upstream {
        None => Some(UpstreamState::Removed),
        Some(a) if a.digest.as_deref() != Some(stored.import_digest.as_str()) => Some(UpstreamState::Outdated),
        Some(_) => None,
    }
}
```

## Testing Strategy

- Local git repos built in tests (as `library`): a commit touching another artifact → not outdated; touching this one → outdated with versions; a branch and subfolder respected; removed upstream; unreachable remote; `ls-remote` short-circuit (no clone when the commit is unchanged — the clone count is observable through `GitConfig`).
- Local source rescans; missing folder.
- Local edits: edited copy detected; Take upstream backs up then replaces; Keep mine skips until the next upstream change.
- Update keeps secrets replaced, re-audits and refreshes links; failure mid-update leaves the old copy.
- Frontend (mockIPC): badge, filter, callout, both dialogs, update all with skipped edits, check errors.

## Boundaries

- **Always:** compare per artifact; back up edited copies; atomic swap.
- **Ask first:** checking anything other than through `git`.
- **Never:** overwrite a local edit without the user's choice; merge; touch projects.

## Success Criteria

1. Only artifacts whose own files changed upstream are flagged, with versions.
2. Updating replaces the library copy atomically, keeps secrets out, re-audits.
3. A locally edited artifact always asks Yours vs Upstream, and the other version is backed up.
4. *Update all* skips edited artifacts and says so.
5. Offline or broken sources are reported per source and never block the rest.
6. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: daily automatic check at start; "Keep mine" silencing until the next upstream change.
