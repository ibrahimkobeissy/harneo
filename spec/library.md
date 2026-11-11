# Spec: library

> Module `library` of the [capability map](capabilities.md). Status: **approved 2026-10-22.**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stages 1–2 (import, library), §4 (feature decisions), §5 (constraints). Visual reference: the validated prototype (`design/prototype/`: `library.js`, the import flow in `flows.js`, the welcome screen in `projects.js`) and `DESIGN.md`.
> Project-wide conventions are in [`app-shell.md`](app-shell.md). This spec records only what this module adds or changes. It builds on [`artifact-model.md`](artifact-model.md): every import starts with `ClaudeAdapter::scan`.

## Objective

Give Harneo its own store of trusted artifacts. The user imports from `~/.claude`, a local folder or a git URL, ticks what to keep, and finds it again later by type, source, tag or collection. Each artifact keeps a record of where it came from, so `updates` and `drift` can later tell what changed.

**User:** the builder described in `CONTEXT.md` §1, including someone who found Harneo on GitHub and has never heard of a profile. For them the library is the home screen and import is the first thing they do.

### Scope

**Core (`harneo-core/src/library/`)**
- **Store** in the data folder:
  - artifacts are copied to `<data dir>/library/artifacts/<id>/`, keeping their paths relative to the artifact;
  - `<data dir>/library/library.db` (SQLite) indexes them.
  The schema has a version number with forward-only migrations.
  - Tables: sources, artifacts (the scanned `Artifact` as JSON, plus columns for filtering), tags, artifact-tags, collections, collection-items.
- **Artifact id**: the first 16 hex characters of SHA-256 over the source key and the artifact's path in the source. It is stable, so a re-import of the same artifact gets the same id.
- **Sources and provenance.**
  - A local source records its absolute path.
  - A git source records the URL, branch, subfolder and **commit** (`git rev-parse HEAD` after the clone).
  - Every artifact also keeps:
    - its path in the source;
    - the scan digest at import;
    - when it was imported;
    - the keys whose secrets were replaced.
- **Import is two steps:**
  1. `preview(source)` scans and returns candidates. Each candidate is an `Artifact`, its id, whether it is **already in the library** (shown unticked and skipped), the secret keys that will be replaced, and suggested tags. The scan's diagnostics come too.
  2. `import(preview, selection, tags)` copies the ticked artifacts and writes the index.
  - **Large files** (decided 2026-10-31): files over the scan's 1 MiB limit are left out and reported as "too large", unless the user ticks *Copy large files too* on the Choose step (one switch per import, off by default). Then they are copied up to 100 MiB each; bigger ones are still left out. No rule reads a file over 1 MiB, so the audit lists them as `unaudited` ("Not audited: over 1 MiB" on the artifact). The choice is kept on the source (`sources.large_files`), so updates copy them too.
  - Each artifact is copied into a temporary folder and renamed into place. The index is written in one transaction. A failed import leaves no half-copied artifact and no index row.
- **Git URLs** use the user's own `git` command:
  - a shallow clone of the branch (`--depth 1`) into a temporary folder in the data dir;
  - then scan the subfolder, then delete the clone;
  - `GIT_TERMINAL_PROMPT=0`, so a private repo without credentials fails instead of hanging, plus a timeout (120 s);
  - accepted forms: `github.com/owner/repo`, `https://…`, `git@…:owner/repo.git`, and GitHub/GitLab `…/tree/<branch>/<subfolder>` URLs. The default branch is used when none is given.
  - The URL is passed as one argument, never through a shell.
  - Clear errors cover a missing `git`, a repo that doesn't exist or can't be reached, an unknown branch, a missing subfolder, a timeout, and nothing found.
- **Literal secrets are replaced, never stored.** The `artifact-model` scan flags literal secrets in Settings `env` and in an MCP server's `env` and `headers`. The library copy gets a placeholder named after the key instead: `${GITHUB_TOKEN}`, or `Bearer ${AUTHORIZATION}` for a bearer header.
  - The import review lists every replacement by key name.
  - The value is never written to the store, the index, logs or IPC.
  - JSON files rewritten this way are pretty-printed. All other files are copied byte for byte.
- **How each type is stored.** An MCP server becomes `server.json`, holding `{ "<name>": { …definition } }`, so one server is one item, as `CONTEXT.md` §2 says. A plugin taken whole (ADR-005, hybrid, 2026-11-08): in its **own folder**, it keeps the whole folder, like Claude Code's install; at the **repo root** (the repo is the plugin), it keeps its component scope, Claude Code's default component folders whole and the top-level entries its own code names (`"templates"`), and no repo plumbing. Nested plugins (their own candidates) and the scanner's skipped folders (`.git`, `node_modules`, `target`, `.trash`) never; the usual large-file, symlink and secret rules; all audited. Every other type copies the files the scan listed for it. For a skill, that includes its scripts. For Settings, it includes its hook and status-line scripts.
- **Tags:**
  - create, rename, recolor and delete them; each has a name and a color from `DESIGN.md`'s tag palette;
  - add and remove them on artifacts.
  - At import, the suggested tags are the artifact's own frontmatter `keywords` or `tags`, when it has them. AI suggestions arrive with `bundles`.
  - Tags live only in the index and are never written into artifact files or projects.
- **Collections:**
  - **manual**: an ordered list of ids;
  - **smart**: a saved rule. A rule combines its rows with AND and the values within a row with OR. The rows are type, tag, source and a text search. Status, trust and invocation rows join when their modules land.
  - Create, rename and delete them; add artifacts to a manual collection and remove them from it.
  - A smart collection is evaluated when it is queried, so new imports join it on their own.
- **Queries:**
  - **List:** filter (types, tags, sources, text over name, description, source and tags), sort (name, type, source, tag count, imported date, in either direction) and page (`offset` and `limit`). The list also returns the total and the counts for each filter value.
  - **Detail:** an artifact's details and its file list.
  - **Remove:** an artifact is removed from the library after a confirmation, deleting its copy and its index rows.

**App (`src-tauri` commands, `src/features/library/`)**
- **IPC commands:** thin wrappers over the core API. Long operations (preview of a git URL) are `async` and never block the window. The types come from `src/bindings/`.
- **Library screen** (home), matching the prototype:
  - a cards view and a list view;
  - the filter column (type, tag, source; it scrolls on its own) and a search box;
  - list columns that sort when clicked;
  - pagination: 24 cards or 25 rows per page, as in the prototype;
  - an empty library leads to the first-run welcome and the guided import.
- **Detail panel:**
  - type, name and description;
  - provenance: the source, and for git the URL, branch, subfolder and commit;
  - the imported date, the files and the scan diagnostics;
  - references, where a resolved one links to that artifact and its Back link returns;
  - tags (add, create or remove) and *Add to… → collection*;
  - Remove.
  - Trust, findings, reach, bundle, usage, invocation mode and update panels appear with their own modules. Until then they are absent, not filled with placeholders.
- **Import flow**, three steps as in the prototype:
  1. Source: `~/.claude`, a git URL (with a branch and subfolder hint) or a folder, chosen in a native folder dialog;
  2. Choose artifacts, grouped by type: "already in library" rows are shown unticked and disabled, and diagnostics show on each row;
  3. Review: suggested tags (accept or remove) and the secrets that will be replaced.
  - Errors and the "nothing found" state match the prototype. The AI bundle and security audit sections of the Review step arrive with `bundles` and `audit`.
- **Collections:**
  - the sidebar's Collections section lists them;
  - a collection screen shows the members with the same filters and pages;
  - the New collection dialog offers manual or smart, with the rule builder for type, tag and source;
  - a smart collection can start from the current filters.
  - *Create profile* from a collection arrives with `profiles`.
- **The Command palette** (Ctrl K) also finds artifacts by name.

**Out of scope:**
- trust and findings (`audit`);
- bundles and AI tag suggestions (`bundles`);
- the outdated flag and updating a copy from its source (`updates`);
- usage, the activity graph and cleanup (`usage`);
- invocation mode, profiles and *Create profile* (`profiles`);
- promote to library, which needs the project registry from `install-engine`;
- the `harneo` CLI commands for the library (`cli`).

## Layout resolver

> Decided by the user on 2026-10-25 after review 1 ([ADR-001](../docs/decisions/ADR-001-layout-resolver.md)). It changes how `preview(source)` decides what a source holds. Tickets are listed in `tasks/plan.md` under review 1.

**Why.** No single template covers how people publish their work. A repo can be a plugin at its root (`addyosmani/agent-skills`), a marketplace pointing at `./` (`mattpocock/skills`) or at subfolders (`pbakaus/impeccable`, a mods repo), a pack of `skills/`, `.claude/skills/` or `.agents/skills/`, or prompts mixed in with docs. Review 1 found three failures of the old discovery: a plugin's whole repo stored as its files (191 files for `mattpocock/skills`), one plugin imported as two items, and plugins that could not be enabled at scaffold. A fixed scanner will always meet a repo it misreads, and Claude can read one the way a person would. But Claude's answer cannot be the source of truth: the repo is untrusted and can steer it, sending content to Claude is opt-in, and updates must read a source the same way every time.

**Principle.** The manifest wins, conventions come next, and Claude only proposes. Whatever decided the layout, the result is validated mechanically, parsed by the normal kind parsers and audited as usual. The resolved layout is stored with the source, so later reads replay it without Claude.

**The three tiers**, run in order for every source (git, local folder, `~/.claude`). A path claimed by a tier is never offered to a later one.
1. **Manifests are authoritative.**
   - `.claude-plugin/plugin.json`: the plugin's files are its manifest and its components. Components are the paths the manifest declares (`skills`, `commands`, `agents`, `hooks`, `mcpServers`, `outputStyles`, `lspServers`), or Claude Code's default folders when a field is absent. Files those components reference inside the plugin (`${CLAUDE_PLUGIN_ROOT}/…`) count as part of them. This component scope decides the pickable parts and what the Library lists as components; a plugin taken whole in its own folder still keeps every other file of that folder, and one at the repo root its default component folders whole and the top-level entries its code names (ADR-005: scripts reach data files through paths built at run time, e.g. `hooks/rules.md`, `templates/settings.json`). Check the field list against Claude Code's plugin reference when building. A `plugin.json` in a marketplace's own folder whose name that marketplace publishes from another folder (impeccable: a working copy at the root, the entry pointing at `./plugin`) is the same plugin: only the marketplace entry, what Claude Code installs, is listed (decided 2026-11-01).
   - `.claude-plugin/marketplace.json`: each entry is one Plugin. An entry whose source is a local path, or a remote source naming the repo being imported, **merges** with the plugin folder it designates and gives it its marketplace (#171). An entry pointing to another repo stays a remote-only Plugin.
   - `installed_plugins.json` in a `~/.claude`-like root, as today.
2. **Known conventions, mechanical.** The shapes in `artifact-model`'s Discovery list: project and `~/.claude`-like children, pack folders at the root, and loose `SKILL.md` folders. Add `.agents/skills/<name>/SKILL.md`, the cross-agent location of the same open SKILL.md format, read as an ordinary Skill. Only Claude's layout is read: any other dot-folder at the scan root (`.opencode`, `.cursor`, `.gemini`, `.codex`, `.kiro`…) is not read for artifacts, and the scan reports each one that holds a skill or plugin once as ignored (`scan.other-agent-folder`: "not read: only .claude and .agents are"). When `.claude/skills/<name>` and `.agents/skills/<name>` both exist, only the `.claude` one is a candidate; the other is reported as ignored (`scan.same-skill-ignored`, "same skill as .claude/skills/<name>"). Claude's layout step (tier 3) never proposes files under an ignored folder. Each artifact records which tier placed it (`metadata.layoutBy`: `manifest`, `convention` or `claude`).
3. **Claude proposes, only for what is left.**
   - **Unplaced files.** After tiers 1 and 2, the scan lists what might still be an artifact: Markdown files with YAML frontmatter holding `name` or `description`, and Markdown under folders named like a kind (`prompts/`, `agents/`, `commands/`, `rules/`…), outside every claimed path. Never counted: `README*`, `CHANGELOG*`, `LICENSE*`, `CONTRIBUTING*`, `.github/`, `.changeset/`, `docs/`, test folders. The scan returns them as `ScanReport.unplaced` (paths only). When nothing at all was found, the whole tree counts as unplaced.
   - **When it asks.** With `unplaced` non-empty, the Choose step shows "N files Harneo couldn't place" with an **Ask Claude what's in this repo** button: an explicit per-use action. The Settings switch `import.layoutWithClaude` makes the call automatic. It is **on by default** (the user's choice, 2026-11-01, an exception to the opt-in rule like the audit's phase 2); turning it off is kept, and its Settings row says what is sent and to whom.
   - **What is sent.** The repo's file tree as paths only (capped at 2,000 paths, the unplaced ones first), the text of any manifests, and the first 40 lines of each unplaced Markdown file (frontmatter and opening), capped at 60 files. Secret-pattern files are listed by name only, never read. The untrusted text goes inside delimited blocks, as in audit phase 2, and text that addresses the reviewer is treated as a threat signal.
   - **What comes back,** through `bundles/claude_cli.rs` with a JSON schema, no tools, no session and no settings: `{ "items": [{ "kind", "name", "path", "reason" }], "ignore": [{ "path", "reason" }] }`. `kind` is one of the 10 types; `path` is a file or folder relative to the root.
   - **Validation.** Every item is checked: the path exists, stays inside the root, crosses no symlink, is not already claimed, and parses with that kind's parser. An item that fails is dropped with a `layout.rejected` diagnostic naming the reason. Claude cannot add files to an item beyond what the kind's parser collects, cannot claim a manifest-placed path, and cannot affect the audit. `ignore` entries only hide paths from the "unplaced" notice.
   - **Failure.** A failed call (no CLI, a limit, an error, a timeout, an invalid answer) **pauses** the Choose step with the reason and three choices: **Retry**, **Continue with what was found** (tiers 1 and 2 only) and **Cancel import**. Nothing is imported until the user picks one.
- **The layout map.** At import, the source stores the resolved layout: for each imported artifact, its kind, path, name and tier. That is the existing artifact record plus `layoutBy`, and the source gets a `layout_version`. Claude's `ignore` list is stored on the source too.
  - **Replay.** `updates` and re-imports run tiers 1 and 2 again (they are deterministic) and replay the stored `claude` entries mechanically: same kind, same path, same parser. Claude is never called during an update.
  - **Layout changed.** When a stored `claude` entry's path is gone or no longer parses, the artifact is flagged *layout changed* in the update check (like *removed*) with an **Ask Claude again** action. New unplaced files at a later check raise the same notice as at import.
- **Choose step.** Each row shows how it was placed: "from plugin.json", "from marketplace.json", "recognised folder" or "suggested by Claude", with Claude's reason on hover or focus. Rows Claude suggested start ticked like the others; the audit's exclusion rules still apply.
- **IPC.** `library_preview_layout(token)` runs tier 3 on a held preview and returns the updated candidates; `layout-progress` events while it runs; `import_layout_setting_get`/`_set`.
- **Out of scope:** adapters for other agents' formats (Cursor rules, Codex `AGENTS.md`) beyond `.agents/skills`; Claude reading whole files; Claude placing anything inside a plugin.

**Testing.** Core tests use the fake `claude` script, never the real CLI:
- fixtures shaped like the four repos in review 1 resolve with tiers 1 and 2 alone and make no call;
- an `.agents/skills` pack is read as Skills;
- a prompts-only repo produces `unplaced`, and a fake answer turns it into Commands;
- a fake answer with a path outside the root, a symlink, a claimed plugin path or an unparsable file has each item rejected with its reason;
- a failing CLI gives the pause state and imports nothing;
- an update replays a stored `claude` entry without calling the CLI, and flags *layout changed* when its path is removed;
- the setting off makes no call unless the button is used.

## Unpacking a plugin

> Decided 2026-11-01 (#198, [ADR-002](../docs/decisions/ADR-002-unpack-plugins.md)). Tickets #199 (scan), #200 (import, storage, updates), #201 (screen).
> **Amended 2026-11-07 ([ADR-004](../docs/decisions/ADR-004-unpack-by-default.md)): unpacked is the default.** A plugin is proposed whole only when it must be (a whole-only part, a part tied by `${CLAUDE_PLUGIN_ROOT}`, or a remote-only marketplace entry), with the reason shown; a whole plugin in the library can be unpacked in one click; plugin handling lives in one core module. Points 1–7 below still hold except the default.

**What.** At import, each plugin is taken **unpacked** (the default since ADR-004) **or whole**: the user picks some of its parts, which become ordinary library artifacts. Never both for one plugin.

1. **Pickable parts:** skills, agents, commands, output styles, and each MCP server of the plugin's `mcpServers` / `.mcp.json`. They are the components tier 1 resolves from the manifest (`plugin_scope.rs`), so no Claude call is involved. Each part carries `metadata.fromPlugin` (plugin name, folder, marketplace) and `layoutBy: "plugin.json"`.
2. **Only with the whole plugin:** hooks, monitors, LSP servers, `bin/`, themes, workflows and the plugin's `settings.json`. They are not listed as parts; the screen says they need the whole plugin.
3. **A part that refers outside itself** (`${CLAUDE_PLUGIN_ROOT}` pointing outside its own files) is listed with a `plugin.part-needs-plugin` error and can't be imported; the reason is shown.
4. **Exclusive per plugin.** In one selection, a plugin and its parts can't both be picked. Across the library, parts can't be imported while the plugin is there whole, and the plugin can't be imported whole while any part is; the refusal names the plugin and what to remove first.
5. **Stored on the source.** `ScanOptions.unpack` lists the plugin folders to unpack; the choice is kept on the source (a migration) and replayed by update checks, so a part updates like any artifact from that source.
6. **Installs as its kind.** An unpacked part is copied into `.claude/…` like any artifact of its kind; the plugin is not enabled in `settings.json`. Unpacked skills lose the `plugin:` namespace; the duplicate-name check covers clashes.
7. **Remote-only marketplace entries** (another repo, never fetched) can only be taken whole.

**Screen.** A plugin row in the Choose step offers **Whole plugin / Pick parts**. Pick parts re-runs the preview with that plugin unpacked and lists its parts nested under it, unticked; parts that can't be picked are disabled with the reason visible; exclusivity refusals show on the row before import. Library rows and the detail panel show "from plugin <name>".

**Out of scope:** enabling part of a plugin through Claude Code; pickable hooks (they would have to merge into a Settings template, which is written whole).

## Tech Stack

New dependencies. They are approved with this spec; any other still needs asking first.

| Dependency | For |
|---|---|
| `rusqlite` with `bundled` (core) | The index. SQLite is compiled in, so there is no new system package and `BUILDING.md` doesn't change |
| `tempfile` (core, now a normal dependency) | Temporary copy and clone folders, cleaned up even on failure |
| `tauri-plugin-dialog` + `@tauri-apps/plugin-dialog` | The native folder picker in the import flow |

`git` must be installed for git URL imports only, checked at run time with a clear message. `scripts/setup.sh` and `BUILDING.md` list it as a runtime prerequisite, since it is already needed to clone Harneo. Versions are the latest stable releases when the first ticket starts.

## Commands

The project commands from `app-shell.md`, plus:

```bash
cargo test -p harneo-core library     # this module's core tests
npx vitest run src/features/library   # its screens
```

## Project Structure

```
crates/harneo-core/src/library/
  mod.rs          → public API: Library::open(data_dir), preview, import, list, detail, remove, tags, collections
  store.rs        → SQLite connection, schema, migrations
  ids.rs          → artifact and source ids
  source.rs       → Source (local | git), git URL parsing
  git.rs          → shallow clone + commit via the git command (the only process spawn in core)
  copy.rs         → per-type copy into the store, atomic rename, secret replacement
  query.rs        → filters, sort, pages, facet counts, smart-rule evaluation
  tags.rs, collections.rs
crates/harneo-core/tests/library_*.rs → integration tests (temp data dir, local git repos built in the test)
src-tauri/src/commands.rs             → library commands
src/features/library/                 → LibraryScreen, ArtifactCard, ArtifactList, FilterColumn, DetailPanel,
                                        ImportFlow, CollectionScreen, NewCollectionDialog, TagMenu, Welcome
src/lib/ipc.ts                        → typed wrappers for the new commands
```

## Code Style

The app-shell conventions apply. The patterns this module adds:

```rust
// crates/harneo-core/src/library/mod.rs
/// Copies the ticked candidates into the store. Each artifact lands whole or not at all,
/// and literal secrets were already swapped for `${KEY}` placeholders in `preview`.
pub fn import(&mut self, preview: &Preview, selection: &[ArtifactId], tags: &TagChoices) -> Result<ImportReport> {
    let tx = self.db.transaction()?;
    // …copy each artifact into a temp dir, rename into artifacts/<id>/, insert its rows…
    tx.commit()?;
    Ok(report)
}
```

- **Every write to the store is atomic:** copy to a temporary folder in the same filesystem and rename into place, with the index rows in one transaction. If the rename succeeds but the commit fails, the folder is removed.
- **`git.rs` is the only place in core that starts a process.** It calls `Command::new("git")` with arguments, never a shell string, and always with the timeout and `GIT_TERMINAL_PROMPT=0`.
- **Paths from a scanned source are untrusted.** Every copied path comes from the scan, which is already relative and inside the root, and is joined under the artifact's folder after a check that it stays there.
- **Frontend:** server state stays in the feature's hooks (`useLibrary`, `useImport`), and filters and sort live in the URL query, so Back keeps them.

## Testing Strategy

The levels follow `app-shell.md`. This module adds:
- **Core integration** (temp data dir):
  - preview, then import, from a fixture folder;
  - a re-import marks every candidate "already in library";
  - an import that fails halfway (an injected copy error) leaves nothing behind;
  - remove deletes the folder and the rows;
  - tags and collections: create, assign, smart-rule evaluation, rename and delete;
  - list filters, sort, pages and facet counts.
- **Git:** each test builds a local repo with `git init` and a commit, and imports it by `file://` URL with a branch and a subfolder, checking that the recorded commit is right. A missing branch, a missing subfolder and a missing `git` binary (`PATH` stubbed) give their errors. No test uses the network.
- **Secrets:** a fixture with a literal MCP header and a Settings `env` value. After import, the stored files contain `${KEY}` placeholders. A test reads every byte of the store and the index and checks that the literal value appears nowhere.
- **Migrations:** opening an empty data dir creates the schema at the current version, and opening it again changes nothing.
- **Frontend** (Vitest, React Testing Library, `mockIPC`):
  - the library list, filters, sort, pages and empty state;
  - the detail panel;
  - each import step, including errors and "nothing found";
  - the tag menu;
  - the new-collection dialog, manual and smart.
- **Performance:** a test library of 2,000 artifacts answers a filtered, sorted page query in under 100 ms.
- **Coverage:** changed lines 80% or more, and the project ratchet must not fall.

## Boundaries

- **Always:**
  - copy into the store atomically;
  - replace literal secrets before anything is written;
  - pass git URLs as arguments;
  - keep tags and collections in the index only;
  - show a module's panels only once that module exists.
- **Ask first:**
  - any change to the store layout or schema once a release has shipped (it needs a migration);
  - a new dependency;
  - running any command other than `git`.
- **Never:**
  - modify a source folder, `~/.claude` or a project;
  - store, log or send a literal secret over IPC;
  - follow a path from a scan outside its artifact's folder;
  - run anything from an imported artifact;
  - leave clone folders or partial copies behind.

## Success Criteria

1. Importing `~/.claude`, a local folder and a git URL (with a branch and a subfolder) each puts the ticked artifacts in the library with full provenance. For git that includes the commit.
2. Importing the same source again shows every artifact as "already in library", and importing nothing changes nothing.
3. A literal secret in an imported MCP server or Settings ends up as a `${KEY}` placeholder in the library copy, the review lists the key, and the value is found nowhere in the data folder.
4. A failed import, whether from a git error or a copy failure, leaves the library exactly as it was and shows a clear error.
5. The Library screen, detail panel and import flow match the prototype in light and dark. Filters, sort and pages work at 2,000 artifacts, with a page query under 100 ms.
6. Tags can be created, colored, added and removed. Manual and smart collections work, and a smart collection picks up a new import that matches.
7. Removing an artifact deletes its copy and its index rows.
8. The source folder, `~/.claude` and any project are never changed. A test checks the fixture's bytes before and after.
9. `npm run check:full` passes and the coverage ratchet holds.
10. **Joint checkpoint with `artifact-model` (2026-10-22):** the user scans their own `~/.claude`, an Addy `agent-skills` clone and the Impeccable plugin with `harneo scan` (artifact-model criterion 8), then imports them through the app. What looks wrong becomes a test and a fix in whichever module owns it.

## Decisions (2026-10-22)

1. **Literal secrets** are replaced in the library copy by `${KEY}` placeholders and listed in the import review. This answers `artifact-model`'s open question and keeps "No secrets stored" (`CONTEXT.md` §5).
2. **The store** is file copies plus a SQLite index (`rusqlite`, bundled).
3. **Git imports** use the system `git` command: a shallow clone, `GIT_TERMINAL_PROMPT=0`, and a timeout.
4. **A re-import** shows "already in library" unticked. Refreshing a copy belongs to `updates`, which protects local edits.
5. **`artifact-model`'s user checkpoint** is merged into this module's (criterion 10), at the user's request.
6. **End-to-end tests of the real window:** still none. Frontend tests with mocked IPC plus the user's manual check (answers `app-shell` decision 2).

## Open Questions

1. **Other agents' layouts** (Cursor rules, Codex `AGENTS.md`): read only `.claude/…` and `.agents/skills` for now, and skip other agents' dot-folders; the rest waits for a second adapter.
