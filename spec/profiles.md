# Spec: profiles

> Module `profiles` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §1 (glossary: profile), §3 Stage 3 (create, clone, Settings template, MCP servers, invocation mode, export/import), §4 (clone replaces nested profiles), §6 (shopping-cart UX). Visual reference: `design/prototype/profiles.js` (list, builder, the files a profile produces), `security.js:60-118` (Settings template summary and view), `library.js` (*Add to… → Profile*, *Used in*, *Manual only*, *Create profile* on a collection).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`library.md`](library.md) and [`bundles.md`](bundles.md) (`bundle_of`).

## Objective

Let the user compose **profiles**: named, versioned selections of library artifacts ("Light", "Heavy", "Rust service") that `install-engine` writes into projects. A profile has any number of artifacts (each pulling its whole bundle), exactly zero or one Settings template, any number of MCP servers with `${VAR}` placeholders, and a *manual only* choice per skill. Building one feels like a shopping cart: the library on the left, the profile and the exact folder tree it will produce on the right.

**User:** the builder who has a library and wants a few sizes of factory. Success: clone *Light*, add two review skills (their subagents come along), save as *Heavy v1*, export it to a file and import it on another machine.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Profiles are versioned.** Every save that changes something writes a new immutable version (v1, v2…); projects pin a version (`install-engine`), and "Profile v5 available" comes from comparing. Saving with no change keeps the version. Name and description edits are not versions.
2. **Bundles are pulled at resolve time, not copied in.** A profile stores what the user picked; `resolve(version)` adds every bundle member with "pulled in by X". Picking X shows its members in the cart at once. Removing a member that a picked item needs asks to remove both (prototype confirm); removing a picked lead asks whether to also remove members no other item needs (default yes) — closing the prototype gap.
3. **Manual only** has a library default (a flag in the index, the drawer toggle; it never edits the library file) and a per-profile override (the builder toggle). The installed `SKILL.md` gets `disable-model-invocation: true` (`install-engine`). Skills only, as in the prototype.
4. **MCP env values must be placeholders** (`^\$\{[A-Z0-9_]+\}$`); anything else is refused with the prototype message and never stored.
5. **Export is one self-contained JSON file**, `<name>.harneo-profile.json`: the profile version plus every artifact it resolves to (provenance, digest, files as UTF-8 text or base64), so the other machine needs nothing else. Library copies already hold placeholders, and export re-checks every file with the literal-secret detector and refuses to write a file containing one.
6. **Removing an artifact from the library** takes it out of every profile, each getting a new version (prototype `library.js:346-358`); projects keep their copy until they switch.

### Scope

**Core (`harneo-core/src/profiles/`)**
- **Schema migration v5:** `profiles(id INTEGER PK, name UNIQUE NOCASE, description, current_version, created_at, updated_at)`, `profile_versions(profile_id, version, spec TEXT JSON, saved_at, PRIMARY KEY(profile_id, version))`, `artifacts.manual_only INTEGER DEFAULT 0`.
- **`ProfileSpec`** (the JSON of a version): `{ items: Vec<ArtifactId>, settings: Option<ArtifactId>, mcp: Vec<McpChoice{ artifact, env: BTreeMap<String, String> }>, manual_only: BTreeMap<ArtifactId, bool> }`. Validation: items are not Settings or MCP; `settings` is a Settings artifact; `mcp` are MCP artifacts; every id exists; env values are placeholders; at least one artifact.
- **API on `Library`:** `profiles() -> Vec<ProfileSummary{id, name, description, version, counts by kind, file_count, project_count (0 until install-engine)}>`, `profile(id, version?) -> ProfileView`, `create_profile(name, description, spec)`, `save_profile(id, spec) -> version` (no-op when equal), `rename_profile`, `delete_profile` (refused while a project uses it, with the project names, once `install-engine` exists), `clone_profile(id, new_name)` ("Cloned from X vN"), `profile_from_collection(collection_id, name)` (items plus their bundles, Settings and MCP left out), `set_manual_only(artifact, bool)`.
- **Resolve:** `resolve(spec) -> Resolved { artifacts: Vec<ResolvedItem{ id, kind, name, why: Picked | PulledBy(ArtifactId) | Settings | Mcp, manual_only }>, missing: Vec<ArtifactId>, worst_trust: Option<u8> }`.
- **Layout:** `layout(spec) -> Vec<PlannedPath{ path, from: Artifact(id) | Generated(HarnessMd | McpJson | Settings) }>`: every project path the profile produces, from `install_path` plus each artifact's own files, `.claude/settings.json` (from the template, plus `enabledPlugins` for plugins), `.mcp.json` when there are servers, and `HARNESS.md`. Two artifacts claiming one path is a `LayoutConflict` shown in the cart. `install-engine` turns this layout into content.
- **Export / import:** `export_profile(id, version) -> Vec<u8>` and `import_profile_file(bytes) -> ProfileImportPreview { profile, artifacts: Vec<{ id, kind, name, in_library: bool }>, name_clash: bool }`, then `import_profile_file_commit(preview, name)`: artifacts not in the library are imported through the library's atomic copy path with their recorded provenance (source rows created as recorded; audit run; secret check), matching by artifact id or content digest; a name clash gets " (imported)". The format has `format: "harneo-profile"` and `formatVersion: 1`; newer is refused; size limit 50 MiB.

**App (`src/features/profiles/`, library additions)**
- **IPC:** `profiles_list`, `profile_get`, `profile_create`, `profile_save`, `profile_rename`, `profile_delete`, `profile_clone`, `profile_from_collection`, `profile_resolve`, `profile_layout`, `profile_export` (native save dialog), `profile_import_preview` (native open dialog), `profile_import_commit`, `artifact_set_manual_only`.
- **Profiles list** (prototype `profiles.js:59-72`): cards with name, `vN`, *Unsaved* badge, description, type counts, "Used in N projects"/"Not used yet", **Clone** and **Export**; **Import profile file** and **New profile** (`N`); empty state.
- **Builder** (`/profiles/:id`, prototype `profiles.js:88-126`): picker (search, type segments, rows with "bundle of N · source · trust T", **+ Add** / **✓ Added**, worst-trust warning toast) and the cart: name, version line ("vN → vN+1 on save"), **Save** (disabled unless changed; unsaved changes kept in the draft while navigating and warned about on leave), *Settings template* (radio list with the prototype summary, eye → settings.json view, *Manage* → Library filtered to Settings, *None*), *MCP servers* (add, remove, env rows with the placeholder guard), *The project it produces* (live tree from `profile_layout`; a file shows "From X · source" or "Generated by Harneo"; *Manual only* toggle on skills; conflicts in red), *Totals*, and the "Used by … projects keep vN" callout.
- **Library additions:** detail panel *Add to… → Profile: X* (adds with bundle, toast as prototype), *Used in* section (profiles with versions), *Manual only* toggle on skills, Invocation filter (Auto / Manual only); collection screen **Create profile**; command palette lists profiles and "New profile"; `N` shortcut.

**Out of scope:** writing projects (`install-engine`); the harness check (`CONTEXT.md` §6); nested profiles (replaced by clone); editing artifact files.

## Tech Stack

New dependency (pre-approved with this spec): `base64` as a direct `harneo-core` dependency, for binary files in the export (it is already in the lock file through Tauri). Native save/open dialogs come from `tauri-plugin-dialog`, already installed.

## Commands

```bash
cargo test -p harneo-core profiles
npx vitest run src/features/profiles
```

## Project Structure

```
crates/harneo-core/src/profiles/
  mod.rs        → ProfileSpec, ProfileSummary, ProfileView, CRUD, versions, clone, from collection
  resolve.rs    → resolve (bundles pulled in), layout (project paths)
  file.rs       → export / import of .harneo-profile.json
  *_tests.rs
src/features/profiles/ → ProfilesScreen, Builder, Picker, Cart, SettingsPanel, McpPanel, ProjectTree, ImportProfileDialog
```

## Code Style

```rust
/// Saves `spec` as the profile's next version, or returns the current one when nothing changed.
pub fn save_profile(&mut self, id: ProfileId, spec: &ProfileSpec) -> Result<u32> {
    spec.validate(self)?;
    let current = self.profile_spec(id, None)?;
    if current == *spec { return Ok(self.current_version(id)?); }
    // insert profile_versions row (version + 1) and bump profiles.current_version in one transaction
}
```

## Testing Strategy

- Versions: save bumps only on change; old versions stay readable.
- Resolve: bundles pulled transitively with `why`; missing artifacts reported; `worst_trust`.
- Layout: paths for each of the 10 kinds, plugins only adding to settings.json, conflicts detected.
- Placeholder guard rejects a literal value; export refuses a file with a literal secret (built from fragments).
- Export → import round trip on a second temp data dir reproduces the profile and imports the missing artifacts with provenance; a newer `formatVersion` is refused.
- Clone, from-collection, delete, library removal creating new versions.
- Frontend (mockIPC): list, builder add/remove with bundle confirm, Settings and MCP panels, tree, save, import dialog, library *Add to profile* and *Used in*.

## Boundaries

- **Always:** validate before saving; placeholders only for MCP env; immutable versions.
- **Ask first:** changing the export format after release (bump `formatVersion`).
- **Never:** store a secret value; edit a library artifact's files to set invocation mode.

## Success Criteria

1. Profiles can be created, cloned, renamed, deleted, created from a collection, and saved as new versions only when changed.
2. Adding an artifact pulls its whole bundle; removing a needed member asks first.
3. The builder shows the exact paths the profile will produce, live, with conflicts flagged.
4. MCP env values are placeholders only; a literal is refused.
5. Export then import on a fresh data dir reproduces the profile and its artifacts.
6. The screens match the prototype in light and dark.
7. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: versions only on content changes; manual-only as a library default plus per-profile override; removing a lead also removes its unshared members by default.
