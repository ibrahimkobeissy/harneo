# Spec: install-engine

> Module `install-engine` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stage 4 (scaffold), Stage 5 (refit, and the WARNING on replacing CLAUDE.md / settings.json), Stage 6.5–6.6 (switch profile, clean removal), Stage 2.4 (promote to library), §5 (whole-file writes, backups before every write, never touch `CLAUDE.local.md` / `settings.local.json`). Visual reference: `design/prototype/flows.js:123-271` (Scaffold and Refit flows), `projects.js:1-108` (Projects list, project detail, Switch / Upgrade modal, clean removal), `profiles.js:9-30` (the files a profile produces).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`profiles.md`](profiles.md) (`resolve`, `layout`), [`library.md`](library.md) (stored copies) and [`artifact-model.md`](artifact-model.md) (`ClaudeAdapter::scan`, `install_path`).

## Objective

Install a profile into a project without ever losing a file. **Scaffold** writes a profile into a folder with no harness; **refit** compares a project's existing harness with a profile item by item and lets the user keep, replace, delete, add or skip each one. Both show a **dry run** with full content (and a diff for refit) before anything touches disk, **back up every file before replacing or deleting it**, write whole files only, and record the project in the **registry** with the profile version and a manifest of what was written. Later the user can **switch** or **upgrade** the profile (only the difference is applied) or **remove** it (originals restored).

**User:** the builder fearful of overwriting a project's setup (`CONTEXT.md` §1). Success: refitting a project whose `CLAUDE.md` has its own deploy notes flags those lines in red, the user keeps that file, the rest is replaced with a backup, and *Remove profile* later puts every original back byte for byte.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Refit rows are items, not single files:** one row per artifact install unit (a skill folder, an agent file, `settings.json`, `.mcp.json`, `CLAUDE.md`…), matched by install path. Actions follow the prototype: in both and different → Keep / **Replace**; identical → **Keep**; project only → **Keep** / Delete; profile only → **Add** / Skip (defaults in bold).
2. **Loss flags:** any replaced or deleted text file whose project version has lines the profile version lacks shows "N lines will be lost" with the lines; `CLAUDE.md` and `.claude/settings.json` rows turn **red** and lead the summary (`CONTEXT.md` WARNING). For settings.json the count is in permission rules and hooks when the JSON parses.
3. **CLAUDE.md:** written only when the profile has a CLAUDE.md artifact. Harneo appends one line, `@HARNESS.md`, when the file does not already import it (prototype `profiles.js:14`). No profile CLAUDE.md → the project's own CLAUDE.md is untouched.
4. **settings.json** is the template's file, plus `enabledPlugins` (and `extraKnownMarketplaces` for a plugin from a non-official marketplace with a repo) for the profile's plugins; with plugins but no template, a minimal file holding only those keys. **`.mcp.json`** is `{ "mcpServers": { … } }` built from the profile's servers with the profile's `${VAR}` env. Both written whole (`CONTEXT.md` §5).
4b. **Plugins are installed after the write** (decided 2026-10-31): settings alone left plugins waiting for Claude Code's trust prompt, which skipped some. After Scaffold or Refit writes the files, Harneo runs the user's `claude` CLI on the project's own `.claude/settings.json`: for each `enabledPlugins` entry it adds the marketplace from `extraKnownMarketplaces` when the machine does not know it (`claude plugin marketplace add`, user settings, as Claude Code's own prompt does; a known marketplace of the same name pointing at another repo is reported, not replaced), then runs `claude plugin install <name>@<marketplace> --scope project` for each plugin not already installed for this project or the user. Plugins come from their own marketplace; nothing is copied or repackaged, no model is called, and the settings file is left byte for byte (checked: the install does not rewrite it). The Done step shows each plugin as installed, already installed, or not installed with the reason, and a Retry; the CLI prints one line per plugin (`--no-plugins` skips it).
5. **Backups:** one backup set per apply, `<data dir>/backups/<project id>/<UTC timestamp>/`, holding each replaced or deleted file at its project path plus `backup.json` (path, digest, existed-before, reason). Backups are kept until the user deletes them from the Backups tab; removal keeps them too.
6. **The project's originals** are the first backup set that holds each path; clean removal restores those and deletes files Harneo created that had no original.
7. **Promote to library** (Stage 2.4) is the library import flow pre-filled with the project's folder; changed managed files are promoted from `drift`.
8. **"Open" a project** opens its folder with the system file manager through `tauri-plugin-opener`.

### Scope

**Core (`harneo-core/src/install/`)**
- **Content:** `render(spec_version) -> Rendered { files: Vec<RenderedFile{ path, bytes, from: Artifact(id) | Generated(kind) }> }` from `profiles::layout`: library copies mapped back to project paths (`copy::stored_view`), a skill with manual-only gets `disable-model-invocation: true` set in its frontmatter (only that key changes; the rest stays byte-equal), settings.json, `.mcp.json`, CLAUDE.md plus the `@HARNESS.md` line, and **HARNESS.md** (profile name and version, date, a table *Artifact · Type · Source · Why it's here* for every artifact, the files, and "Managed by Harneo: edit the library copy, or your edits show as drift.").
- **Scaffold:** `plan_scaffold(target, profile, version) -> Plan`. Refused when the folder has a harness (`.claude/`, `CLAUDE.md` or `.mcp.json`) with `HasHarness` ("…refit it instead"), when it is not a directory, or when a path is not writable (checked before writing).
- **Refit:** `compare(target, profile, version) -> Comparison { rows: Vec<Row{ item, kind, in_project: Absent|Same|Differs|Exists, in_profile, default_action, options, loss: Option<Loss{ lines, items }> }> }` from `ClaudeAdapter::scan(target)` and `render`. A registered project on the same version with no difference → `NothingToChange`. Then `plan_refit(comparison, choices) -> Plan`.
- **Plan:** `Plan { target, profile, version, writes: Vec<Write{ path, action: New|Replace|Delete|Keep|Skip, bytes, diff: Option<Vec<Hunk>> }>, backup_count }`. Diffs come from `similar` (line diff, 3 lines of context).
- **Apply:** `apply(plan) -> ApplyReport`:
  1. take the data-dir lock (`std::fs::File::lock`, shared with the CLI);
  2. back up every Replace/Delete target into a new backup set — if any backup fails, nothing is written;
  3. write each file atomically (temp file in the same folder, then rename), creating folders as needed; delete Delete rows;
  4. on any write error, restore from this backup set and delete files this apply created, then return the error ("Nothing was changed" / "Rolled back");
  5. record the project, profile version and manifest (path, digest, artifact id) in one transaction.
  Paths are checked to stay inside the target; Harneo never writes through a symlink (it refuses the plan), and never reads, writes or deletes `CLAUDE.local.md`, `settings.local.json` or anything under `.git/`.
- **Registry (schema migration v6):** `projects(id, path UNIQUE, name, profile_id, profile_version, applied_at)`, `installed_files(project_id, path, digest, artifact_id, PRIMARY KEY(project_id, path))`, `backups(id, project_id, created_at, reason, file_count, folder)`. API: `projects()`, `project(id)`, `installed_files(id)`, `backups(id)`, `restore_backup(backup_id)` (current versions backed up first), `delete_backup`.
- **Switch / upgrade:** `plan_switch(project, profile, version) -> Plan`: compares the manifest with the new render; adds, updates and removes only the difference; a removed path that replaced an original is restored from the originals, otherwise deleted; a file edited in the project since apply is flagged "edited in project" and backed up before replacement.
- **Clean removal:** `plan_removal(project) -> Plan` then `apply`: deletes Harneo's files (backing up any that were edited), restores originals, removes `HARNESS.md`, and removes the project from the registry.
- `ProfileSummary.project_count` and *Used in N projects* now come from the registry; deleting a profile in use is refused.

**App**
- **IPC:** `scaffold_plan`, `refit_compare`, `refit_plan`, `install_apply` (async), `projects_list`, `project_get`, `project_files`, `project_file_content`, `project_backups`, `backup_restore`, `backup_delete`, `switch_plan`, `removal_plan`, `project_open` (opener).
- **Scaffold flow** (`/scaffold`, prototype `flows.js:123-183`): Folder (native dialog, "has a harness" error with **Refit it instead**), Profile (cards with file counts; empty → Create a profile), Preview (dry-run callout, tree with every file *new*, code view of the full content, write-error callout), Done (Open project, Scaffold another).
- **Refit flow** (`/refit`, prototype `flows.js:185-271`): Folder (registered projects listed; "no harness → Scaffold it instead"), Profile (current one preselected for a registered project), Compare (sortable table *File · In project · In profile · Action*, red loss rows with Show/Hide of the lost lines, top loss callout, summary "Replace N · add N · delete N · keep N · skip N. N files backed up first…"), Preview (tree marked new/replace/delete, +/− diff), Done (Restore originals, Go to projects).
- **Projects** (`/projects`, `/projects/:id`, prototype `projects.js:16-108`): list cards with status pill (*In sync* / *Profile vN available*; *Drift* arrives with `drift`), detail with Open, Switch profile, Remove profile, the upgrade callout, stat panels, tabs *Installed files* (tree + viewer) and *Backups* (Restore, delete), the Switch / Upgrade modal (Add / Remove / Update groups, "Apply difference"), and the clean-removal confirm with the prototype copy.
- **Promote:** project detail **Import from this project** opens the import flow with the folder.
- Sidebar Projects count; command palette lists projects.

**Out of scope:** drift detection and live rescan (`drift`); merging file contents (never); installing into `~/.claude` (never); updating library copies (`updates`).

## Tech Stack

New dependencies (pre-approved with this spec): `similar` (core, line diffs for refit, switch and later `updates`/`drift`), `tauri-plugin-opener` + `@tauri-apps/plugin-opener` (open a project folder). The data-dir lock uses `std::fs::File::lock` (stable Rust). Latest stable versions when the first ticket starts.

## Commands

```bash
cargo test -p harneo-core install
npx vitest run src/features/scaffold src/features/refit src/features/projects
```

## Project Structure

```
crates/harneo-core/src/install/
  mod.rs        → Plan, Write, ApplyReport; apply (lock, backup, atomic writes, rollback)
  render.rs     → content for each layout path; HARNESS.md; settings/.mcp.json generation; frontmatter edit
  compare.rs    → refit comparison, loss detection, choices → plan
  registry.rs   → projects, installed files, backups; switch and removal plans
  backup.rs     → backup sets, restore
  *_tests.rs
src/features/scaffold/, src/features/refit/, src/features/projects/
```

## Code Style

```rust
/// Writes a plan into its project. Every file that is replaced or deleted is backed up first;
/// a failure part-way restores the backup and removes what this apply created.
pub fn apply(&mut self, plan: &Plan) -> Result<ApplyReport> {
    let _lock = DataLock::acquire(&self.data_dir)?;
    let set = backup::create(&self.data_dir, plan)?;        // nothing written if this fails
    match write_all(plan) {
        Ok(written) => self.record(plan, &set, written),
        Err(e) => { backup::rollback(&set, plan)?; Err(e) }
    }
}
```

## Testing Strategy

- Render: every kind's paths and bytes; manual-only frontmatter edit leaves the rest byte-equal; HARNESS.md; settings with plugins; `.mcp.json` env placeholders; CLAUDE.md `@HARNESS.md` added once.
- Scaffold: plan on an empty folder; refused with a harness; unwritable folder (read-only temp dir) writes nothing.
- Refit: each row kind and default; loss detection for CLAUDE.md lines and settings.json rules; choices change the plan.
- Apply: backups before writes; injected write failure rolls back to the exact original bytes; symlink target refused; `CLAUDE.local.md` / `settings.local.json` untouched (bytes compared).
- Switch: only the difference; restore of a replaced original; edited file flagged.
- Removal: every original restored byte for byte and every Harneo-only file gone (a property test over random harness trees).
- Frontend (mockIPC): both flows step by step incl. errors, projects list and detail, switch modal, removal confirm, backups restore.

## Boundaries

- **Always:** dry run first; backup before replace or delete; whole-file atomic writes; manifest recorded.
- **Ask first:** writing anywhere but the chosen project and Harneo's data folder.
- **Never:** merge file contents; touch `CLAUDE.local.md`, `settings.local.json` or `.git/`; write through a symlink; delete a backup without the user.

## Success Criteria

1. Scaffolding writes exactly the previewed files and registers the project.
2. Refit shows every item with the right default; CLAUDE.md and settings.json losses are flagged in red with the lines.
3. Every replaced or deleted file is in a backup set before it changes; a failed apply leaves the project as it was.
4. Switch and upgrade apply only the difference; clean removal restores every original byte for byte.
5. `CLAUDE.local.md`, `settings.local.json` and anything outside the project are never touched (tests).
6. The flows and project screens match the prototype in light and dark.
7. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: the `@HARNESS.md` line appended to CLAUDE.md; backups kept forever until deleted; item-level refit rows.
