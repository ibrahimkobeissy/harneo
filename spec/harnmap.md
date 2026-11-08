# Spec: harnmap

> Module `harnmap` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-11-07): "create the spec and let's start devving"**, after the UX was validated in Claude Design. Ticket [#224](https://github.com/ibrahimkobeissy/harneo/issues/224) (UX phase). Built on the `feat/harnmap` branch, merged to `main` when the user accepts it.
> Product context: [`CONTEXT.md`](../CONTEXT.md) §1 (glossary: category, Harnmap, Harnfit), §3 Stage 3.7–3.9, §4, §6 (harness check: this is its judgement half).
> Visual reference: **`design/harnmap/`** (the validated Claude Design boards: `Main`, `NewProfile`, `FirstDraw`, `Harnmap`, `HeavyMap`, `Harnfit`, `Templates`, `VideoMap`, `EntryPoints`, `Export`, `States`, and `canvas.json` for the decisions note). Background: the first idea and its feasibility review were drafts, retired in #267 once built (git history has them); every decision in them is settled below.
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`profiles.md`](profiles.md) (profiles, versions, export file), [`bundles.md`](bundles.md) (links, `claude_cli`, settings store), [`library.md`](library.md) (artifacts, import preview).

## Objective

Let the user **see a profile as a factory** before using it, shape that picture by hand, and **try a new artifact against it** before adding it.

- **Category:** every profile has one (8 choices). It picks a **template**: a skeleton of stages and a **shape** of flow.
- **Harnmap:** Claude (through the user's `claude` CLI) places the profile's artifacts on the template: on the **line** (a stage), the **toolbelt** (any stage) or the **floor** (always on), with a short note and its reasoning. Harneo draws the result as an interactive diagram. The user moves, pins, notes, hides and removes nodes, and edits stages. Every drawing is kept in a history; nothing is ever lost.
- **Harnfit:** for one candidate artifact (from the library or the import review), Claude proposes where it would go on the profile's map and gives a verdict (fits, fits with changes, conflicts) with quotes Harneo checks word for word.

**User:** a builder with a few profiles who keeps finding new skills online. **Success:** they open *Heavy → Harnmap*, press Draw, and in 1–3 minutes see their factory; they drag `webperf` into Test, pin it, re-draw with a new description and it stays put; they Harnfit `tdd-lite` and see it conflicts with `test-driven-development`, with both lines quoted; they export Heavy and the map arrives on another machine with no tokens spent.

### Decisions (user, 2026-11-05 to 2026-11-07)

1. **Three layers:** line (skills, commands, subagents, workflows), toolbelt (cross-cutting skills, MCP servers), floor (CLAUDE.md, rules, settings template, output style). Claude may put any artifact anywhere; the user overrides.
2. **Stages come from the category's template**, adjusted by Claude (rename, add, remove; it says so) and by the user. Claude never invents a shape: it picks one of the five.
3. **One map per profile**, saved with it. It doesn't change the profile's version.
4. **Nothing is lost:** every edit autosaves; a re-draw never overwrites (history, restore as a new draw); Harnfit verdicts are saved; export, import and clone carry the map, its history and the fits.
5. **Model: Opus by default, Sonnet allowed, never Haiku.** Its own setting, `harnmap.model`. If the account can't use the chosen model, offer the other one; never fall back silently. Each draw and fit records its model and duration.
6. **Consent:** pressing **Draw**, **Re-draw**, **Harnfit** or **Run again** is the go-ahead, with what is sent listed next to the button. No switch before it. No automatic Harnfit at import in v1.
7. **Facts vs Claude's reading:** everything Harneo knows from the files (bundles, frontmatter fields, hooks, rule paths, manual-only) is shown as a fact; everything Claude proposes is labelled as Claude's. Whatever Harneo can compute (changes from the template, history change lines, out-of-date lists, line numbers of quotes) is computed, never asked of Claude.
8. **Harnfit: one candidate at a time**, from the library or the import review. In v1 a candidate is a single artifact; plugins come in v1.1.
9. **v1 cut** (canvas note, 2026-11-07): templates; draw with an honest wait and a replay (no live placement); the map with editing, pins, history and a list view; Harnfit for one artifact. **v1.1:** plugin Harnfit, toolbelt wires, per-profile social media lanes, live placement (needs a spike on `stream-json`), the full factory decoration (chutes, valves).
10. **Template changes from the design are accepted** (Research Verify → Sources loop; Video "Shoot & assets"; Data Model → Explore loop; Design adds Define and a Prototype ⇄ Critique loop; Social media platforms are plain stages under Adapt in v1).
11. **A fit keeps only its latest verdict.**
12. **No graph library.** Nodes never move freely: they sit in a zone (a stage, the toolbelt, the floor) and the layout is computed per shape. Pan, zoom and drag-to-zone are a small canvas of our own (HTML nodes over an SVG layer). No new dependency.

## Scope

### Core (`crates/harneo-core/src/harnmap/`)

**Templates (`templates.rs`)**: static data, one per category: `Category { id, label, shape, stages: Vec<{ id, name }>, loops: Vec<{ from, to, label }>, phases?: Vec<{ name, stages }>, parallel: Vec<StageId>, suggestions: [String; 3] }`. `parallel` lists the stages drawn side by side as tracks between a fork and a join (`video`: Edit and Sound & captions); empty elsewhere.

| Category id | Shape | Stages (loops) |
|---|---|---|
| `software` | `line` | Idea, Spec, Plan, Build, Test, Review, Ship (Review → Build "changes requested") |
| `research` | `line` | Question, Sources, Read & extract, Analyse, Synthesise, Verify, Report (Analyse → Read & extract; Verify → Sources) |
| `writing` | `line` | Brief, Research, Outline, Draft, Edit, Fact-check, Publish (Edit → Draft; Fact-check → Draft) |
| `video` | `parallel` | Concept, Script, Storyboard, Shoot & assets, Edit ∥ Sound & captions, Publish (phases: pre-production, production, post-production) |
| `social` | `cycle` | Plan, Ideate, Create, Adapt per platform, Schedule, Engage, Measure (Measure → Plan) |
| `data` | `line` | Question, Collect, Clean, Explore, Model, Visualise, Report (Clean ⇄ Explore; Model → Explore) |
| `design` | `diamond` | Brief, Research, Define, Concepts, Prototype, Critique, Hand-off (Critique → Prototype) |
| `custom` | chosen by Claude | One open stage until the first draw |

Shapes: `line`, `cycle`, `diamond`, `parallel` and, for Custom only, any of these. Loops are drawn on any shape, so "line + loop" is a `line` with loops. Each template has three suggestion chips for the draw description (`software`: "Spec first, always.", "TDD lives inside Build.", "Review sends work back to Build."; the others are written in the same spirit).

**Schema migration V12:**
- `profiles.category TEXT NOT NULL DEFAULT 'software'`, `profiles.category_note TEXT` (Custom's description).
- `harnmap_draws(id INTEGER PK, profile_id REFERENCES profiles ON DELETE CASCADE, no INTEGER, created_at, model, duration_ms, description, category, map TEXT JSON, basis TEXT JSON, restored_from INTEGER NULL, UNIQUE(profile_id, no))`. The row with the highest `no` is the **current map** and is the only one edits change; older rows are frozen.
- `harnfit_fits(profile_id REFERENCES profiles ON DELETE CASCADE, candidate TEXT, created_at, model, duration_ms, verdict TEXT JSON, basis TEXT JSON, PRIMARY KEY(profile_id, candidate))`.

**Map document (`map.rs`)**, the JSON in `harnmap_draws.map`:

```rust
pub struct MapDoc {
    pub category: CategoryId,
    pub shape: Shape,
    pub stages: Vec<Stage>,          // { id, name, origin: Template | Claude | User }
    pub loops: Vec<Loop>,            // { from: StageId, to: StageId, label }
    pub phases: Vec<Phase>,          // copied from the template
    pub parallel: Vec<StageId>,      // stages drawn as parallel tracks
    pub placements: Vec<Placement>,  // one per placed artifact
    pub hidden: Vec<ArtifactId>,
    pub hidden_types: Vec<ArtifactKind>,
    pub changes: Vec<TemplateChange>, // computed by Harneo after a draw
}
pub struct Placement {
    pub artifact: ArtifactId,
    pub zone: Zone,                  // Stage(StageId) | Toolbelt | Floor
    pub order: u32,
    pub note: String,                // ≤ 60 chars, shown under the name
    pub why: String,                 // Claude's reasoning, ≤ 400 chars
    pub role: String, pub when: String, pub hands_off: String, // heartbeat tooltip, ≤ 120 chars each
    pub next: Vec<ArtifactId>,       // Claude's natural next steps, most likely first, ≤ 4 (2026-11-08)
    pub next_why: BTreeMap<ArtifactId, String>, // why each is a next step, ≤ 120 chars, Claude's reading (#263)
    pub parent: Option<ArtifactId>,  // the artifact it only runs through (Claude's reading, #251)
    pub also_run_by: Vec<ArtifactId>, // other artifacts that also run it, ≤ 3 (#251)
    pub by: PlacedBy,                // Claude | User
    pub pinned: bool,
}
```

Unplaced artifacts are not stored: they are the profile's resolved artifacts with no placement. `basis` records what the map was drawn from: profile version and `(artifact id, digest)` for every resolved artifact.

**Facts (`facts.rs`)**: `facts(profile) -> BTreeMap<ArtifactId, Vec<Fact>>`, from the files only: bundle links in both directions (`links` table), a subagent's `skills` and `mcpServers`, a skill's or command's allowed `mcp__<server>__*` tools, `disable-model-invocation` / manual-only, `user-invocable: false` / model-only (#251), a rule's `paths`, the settings template's permissions count, hooks (event, matcher, command) and status line, CLAUDE.md "loaded in every session", output style "applies to every reply", and **missing MCP servers** (a subagent or skill naming a server the profile doesn't include). Also `bundles(profile) -> Vec<Vec<ArtifactId>>` for the shaded groups.

**Draw (`draw.rs`)**:
- `draw(profile, description, model) -> Result<DrawOutcome>`, run on a worker thread like `bundles` detection, cancellable (the `claude` process is killed; nothing is saved).
- The prompt holds: the template (or Custom's description and the five shapes), the current stages with the user's edits by id, pinned placements as constraints, the description, and per artifact a short id (`a1`…), kind, name, description, facts, and its main-file text cut to 4,000 characters (artifacts with a `plaintext-secrets` or `credential-theft` finding: kind, name and description only, as in `bundles`). Artifact text is data; the prompt says so.
- `claude_cli::run` with `--json-schema` (the answer schema below), model from `harnmap.model` (`opus` by default; only `opus` and `sonnet` are accepted), a **300 s timeout**, no tools, the same flags as today.
- **Answer:** `{ shape?, stages: [{ id?, name }], loops: [{ from, to, label }], placements: [{ artifact: "a3", zone: "stage:<name>" | "toolbelt" | "floor", note, why, role, when, hands_off, next: [{ "artifact": "a4", "why": "…" }, …], parent?: "a2", also_run_by?: ["a5"] }] }`. `next` (added 2026-11-08) names the **immediate next step** after the artifact: alternatives a user reaches for right after it, most likely first, at most 3 asked (4 kept for older maps), never a chain of the rest of the workflow (#255, user decision 2026-11-08), each with a one-line `why` (≤ 120 characters: what in this artifact leads to that one; #263, user request 2026-11-08); unknown ids, the artifact itself and repeats are dropped, at most 4 kept. `parent` and `also_run_by`: see *Parent and child* below.
- **Validation:** every artifact id is known and placed at most once; every stage named in a placement or loop exists; the shape is one of the five; lengths are cut to the limits. An answer that fails is **retried once**; a second failure is `HarnmapError::BadAnswer { reasons }` (shown as "Answer didn't check out") and nothing is saved.
- **Pins and stage edits win:** Harneo re-applies every pinned placement and every user-edited stage (by id) to the validated answer, whatever Claude returned.
- **Changes from the template** are computed: stages kept, renamed, added, removed; loops added; count unplaced.
- Saved as a new draw (`no + 1`) with model, duration and basis. Failures are `ClaudeFailed { kind }` with the existing kinds (not signed in, limit, model unavailable, timeout, other) plus `ClaudeMissing`.

**Parent and child (#251, user decisions 2026-11-08)**:
- Many packs ship a command that does nothing but run a skill (`/spec` → `spec-driven-development`), or a skill or subagent only ever run by one other artifact. **Claude alone judges the relation** in the draw: the prompt asks, per placement, for a `parent` only when the artifact is not meant to be started on its own and runs through that parent. Harneo detects nothing itself; the relation is always labelled "Claude's reading". The files' facts (`ManualOnly`, `ModelOnly`, bundle links, a subagent's `skills`) are in the prompt as evidence.
- **Kinds:** a parent is a command, a skill or a subagent; a child is a skill or a subagent. Any other pair is dropped.
- **One parent:** an artifact run by several others nests under the one Claude names as `parent`; the others go in `also_run_by` (≤ 3, shown as "Also run by /x"). Without a parent, `also_run_by` is empty.
- **Two levels at most:** command → skill → subagent. A parent whose own parent already has a parent is dropped (the artifact draws as an ordinary node).
- **Validation** (with the rest of the answer, the retry rule unchanged): an unknown, unplaced or self `parent` is dropped, so is one that makes a cycle (the later placement in answer order loses it); unknown ids, the artifact itself, its parent and repeats are dropped from `also_run_by`.
- **The child goes with its parent:** Harneo sets a child's zone to its parent's, whatever Claude answered, and keeps it so: `Move` of a parent moves its children (and theirs) with it; `Remove` of a parent, or hiding it, turns its children into ordinary placements in the same zone, after the parent's place. `RemoveStage` sends parents and children to Unplaced together.
- **Detach:** `MapEdit::Detach { artifact }` clears `parent`, keeps the child in the same zone right after its parent, and pins it (`by: User`). `Move` of a child also detaches it. A detached child stays detached on Re-draw, like any pin. A pinned child that keeps its parent is re-applied with it.
- **Model-only:** a skill with `user-invocable: false` gets the `ModelOnly` fact ("Model only: user-invocable is false"), the mirror of `ManualOnly`. It comes from the files and needs no parent.
- Old maps read as having no parents. Export, import and clone map `parent` and `also_run_by` to keys like `next`; a key that doesn't resolve is dropped.

**Map API (`mod.rs`)** on `Library`: `set_category(profile, category, note)` (the map is then out of date, nothing is sent); `harnmap(profile) -> Option<MapView { doc, no, created_at, model, duration_ms, description, facts, bundles, unplaced, stale: Option<Staleness> }>`; `harnmap_edit(profile, edit: MapEdit)` with `Move { artifact, zone, order }` (pins it), `Place { artifact, zone, note }` (pins it), `Remove { artifact }`, `Unpin { artifact }`, `Note { artifact, text }`, `Hide / Show { artifact }`, `HideType / ShowType`, `RenameStage`, `AddStage { after, name }`, `RemoveStage { id }` (its stations go to Unplaced unpinned; its loops go), `MoveStage { id, to }`; `harnmap_history(profile) -> Vec<DrawSummary { no, created_at, model, duration_ms, description, change_line, restored_from }>` (the change line is computed by comparing a draw with the previous one); `harnmap_draw_at(profile, no) -> MapDoc` (preview); `harnmap_restore(profile, no) -> no` (copies it as a new draw).
- **Staleness:** comparing `basis` with the current resolve: artifacts added (they wait in Unplaced), removed (their placements are dropped from the view), changed (digest differs), and category changed.

**Harnfit (`fit.rs`)**:
- `fit(profile, candidate: Candidate, model) -> Result<FitView>`; `Candidate::Library(ArtifactId)` or `Candidate::Preview { preview_id, candidate }` (from an import preview, read the same way `detect_for_preview` does). Refused for a plugin taken whole (v1.1) and when the profile has no map (`HarnmapError::NoMap`).
- The prompt holds the current map (stages, placements with notes), the candidate in full (main file and supporting text files, 12,000 characters), and the profile's artifacts' main files: 4,000 characters each, **same kind as the candidate first**, up to 120,000 characters in all; past that, kind, name and description only.
- **Answer:** `{ verdict: "fits" | "fits_with_changes" | "conflicts", zone, why, change?, conflicts_with: ["a4"], quotes: [{ artifact: "candidate" | "a4", text }] }`.
- **Quotes are checked:** Harneo looks for each quote, whitespace-normalised, in that artifact's files and records the file and the line it finds. A quote it can't find is dropped and the verdict says "1 quote couldn't be checked and was removed".
- **Missing MCP servers** come from `facts`, not from Claude: the verdict lists them, with `in_library: bool`.
- Saved in `harnfit_fits` (latest only) with model, duration and basis: the profile's resolve digests and the candidate's digest and source (library id, or source URL and commit for a preview). A fit is **out of date** when either changed.
- `fits(profile) -> Vec<FitSummary>`, `fits_for(artifact) -> Vec<(ProfileId, Verdict, stale)>` (for the library card badge), `fit_add(profile, candidate)`: adds the artifact to the profile (a new profile version, through `save_profile`) and places it on the map at its proposed zone, pinned, with the fit's `why` as its note. A preview candidate must be imported first (the button says so).

**Export / import (`profiles/file.rs`)**: `formatVersion` 2. The profile file gains `harnmap: { draws: [...] }` (current draw always; earlier ones when "Include map history" is on) and `fits: [...]` (when "Include saved fits" is on). Inside the file a map refers to artifacts by **key** (`kind`, `name`, `digest`), not by id; import remaps keys to library ids and drops placements whose artifact isn't in the profile. The map part is untrusted: size-capped with the file, schema-checked, its text fields cut to the limits, and shown as plain text only. Version 1 files still import (no map). `clone_profile` copies the category, all draws and the fits.

**Settings keys:** `harnmap.model` (`opus` | `sonnet`, default `opus`).

### App (`src/features/harnmap/`, with additions to `profiles`, `library`, `import`, `settings`)

- **IPC:** `profile_set_category`, `harnmap_get`, `harnmap_draw` (async, cancellable through `harnmap_cancel`), `harnmap_edit`, `harnmap_history`, `harnmap_draw_at`, `harnmap_restore`, `harnfit_run` (async, cancellable), `harnfit_list`, `harnfit_for_artifact`, `harnfit_add`, `templates_list`. `profile_export` takes `{ includeHistory, includeFits }`.
- **New profile** (board `NewProfile`): the category radio cards with a shape thumbnail and one line each, Custom's description field, the stage preview; *Create profile*.
- **Profile builder** (board `Main`): a category control (menu; a change asks first: "Nothing is sent now. The map is marked out of date…"), a **Harnmap** button, and the **Harnmap card** (mini preview, state badge Up to date / Out of date / Not drawn yet, "Drawn <date> · N drawings in history · X of Y artifacts placed", *Open Harnmap*).
- **Harnmap screen** (`/profiles/:id/harnmap`, boards `FirstDraw`, `Harnmap`, `HeavyMap`, `VideoMap`):
  - Header: breadcrumb, category badge, "Draw N · date · model · duration", saved status, **Map / List** switch, **Stages**, **Map history**, **Re-draw**, close.
  - **Not drawn yet:** the template skeleton; the side panel with the description box, the template's three suggestion chips, the facts (Sends N artifacts · Through your claude CLI · Uses your plan · Opus · Takes usually 1–3 min), the "What is sent / Never" line, **Draw** (`Ctrl+Enter`).
  - **Drawing:** an honest wait ("Claude is working · Opus", elapsed clock, Cancel). When the answer lands and passes the checks, the placements **replay** onto the line in about 3 s (each artifact rides to its zone; the stage light comes on). Then "Draw N · saved", the computed changes from the template, *Open the map*.
  - **The map:** the layout per shape (below); stage blocks with number, name, count and light; loops as dashed arcs labelled "Claude's reading" unless the user drew them; the settings template's hooks as a rail along the whole line, labelled with event and matcher; bundles as shaded groups; the toolbelt and floor zones; nodes with type icon, name, note, pin mark; the **heartbeat dot** (slate, one shared 2 s beat, blue and still while its tooltip is open, not a tab stop; `i` on a focused node opens the tooltip; Escape closes); the tooltip with Role, When and Hands off, each tagged *files* or *Claude*; a legend (bundle · from the files, Claude's reading).
  - **Next steps** (2026-11-08): selecting a node rings its immediate next steps (Claude's reading: alternatives, not a sequence), numbered by likelihood with a small "Next steps · most likely first" caption; the node detail ends with a **Natural next steps** section listing each step (number, type icon, name, Claude's one-line reason, clickable to select it), or the steps alone for a map drawn before reasons existed (#263), with a dashed arrow from the selection to each.
  - **Parent and child** (#251): a child draws right under its parent in the same stack (stage, toolbelt or floor), narrower and indented one step per level (two at most), with a solid connector from the parent to it; a parent and its children stay together in one column of a busy stage and move together when the parent is dragged. The child's tooltip and node detail say "Runs only through /spec · Claude's reading" and "Also run by /x" when set; the node detail has **Detach** (dragging the child elsewhere also detaches it). A **model-only** skill (fact `ModelOnly`) carries a small "model only" mark on its node and a *files* line in its tooltip. The list view indents children under their parent.
  - Canvas: zoom 40–150 % (buttons, `Ctrl` + wheel), drag the background to pan, **Fit**; below 75 % the map turns into plain boxes (no beam, no rings); layers switch (Line, Toolbelt, Floor) and **Types** menu with counts.
  - **Side panel:** *Unplaced* (tree by type, filter, "Not everything is a step…", drag onto a stage, the toolbelt or the floor, then the "Why here?" popover; *Hidden on the map* with Show; *On the map* summary); **node detail** (Why Claude put it here / Why here, *From the files* facts, description, note (autosaves), *Move to* select, Remove from map, Hide, Unpin); **Map history** (each draw with date, model, description, change line; Preview, Restore).
  - **Stages** popover: rename, reorder by drag, add, remove, with the rules above.
  - **Re-draw** dialog: the description, "Stays put: N pinned" list, the what-is-sent line, *Draw again*. The current map stays visible while Claude works.
  - **List view:** a table of artifact, layer, stage, note, placed by (Claude / You, pinned) for every placed artifact, then the unplaced ones.
- **Layout (`src/features/harnmap/layout/`)**, pure functions, one per shape, from `(stages, loops, phases, placements by zone, node size)` to positions of stage frames, nodes, loop arcs, the hook rail, bundle groups and zones:
  - `line`: stages left to right, stations stacked under each stage, loops as arcs below the line.
  - `cycle`: stages around an ellipse, the return arrow closing it.
  - `diamond`: two diamonds (Brief…Define, Define…Hand-off), stages on their edges.
  - `parallel`: stages left to right, the parallel pair stacked as two tracks between a fork and a join, phases as zones behind.
  - Every shape: any number of stages (1–12) and stations per stage, long names ellipsised, toolbelt and floor as bands below. The decoration (stage lights, the beam along the line) is generated from the same positions.
- **Harnfit screen** (`/profiles/:id/harnfit/:candidate`, board `Harnfit`): the profile's map faded; the candidate as a dashed "NEW" node at its proposed zone; the verdict card (Fits green / Fits with changes amber / Conflicts red) with where it would go, why, the proposed change, the checked quotes (file:line, "checked word for word against the files"), conflict links drawn red to the artifacts it clashes with, missing MCP servers ("isn't in your library; import it first" or "Add with it"); actions **Add to profile** (primary), **Place elsewhere** (drag the dashed node; saved with the fit), **Discard** (the fit stays saved); "This is Claude's opinion"; *Saved fits for <profile>* list; header "Fit saved <date> · model · duration", **Run again**.
- **Entry points** (board `EntryPoints`): the library card and detail panel menu **Harnfit in…** → pick a profile (each row: category, "fit saved <date>" with its verdict badge, or "not fitted yet"); the library card shows a verdict badge per profile with a saved fit (marked out of date when stale); the **import review** gets a profile select and a **Harnfit** button per candidate.
- **Export dialog** (board `Export`): rows for the map, *Include map history*, *Include saved fits* (both on by default), the file name `<name>.harneo-profile.json · format v2 · size`. The import result says "Its Harnmap is ready… No tokens spent".
- **States** (board `States`): empty profile (template only, Add from library, Edit stages); `claude` CLI not found (official setup guide link, Check again); not signed in; usage limit; model unavailable (Draw with Sonnet / Opus); timed out (Try again, Use Sonnet); answer didn't check out (Try again, Show the answer); nothing on the line; out of date (+ / − / ~ list, Re-draw, Place by hand); category changed (Re-draw on <category>, Keep the old map for now); Harnfit needs a map (Draw <profile>'s map first); fit out of date (Run again, Keep the old verdict). Failures leave the current map unchanged.
- **Settings screen:** *Harnmap and Harnfit model* select (Opus, Sonnet) under the Claude CLI section.

### Out of scope (v1)

Plugin Harnfit, toolbelt wires, social media platform lanes, live placement while Claude works, chutes and valves, automatic Harnfit at import, a minimap, a `harneo map` / `harneo fit` CLI.

## Tech Stack

No new dependency. Everything runs on the existing stack (Tauri 2, React 19, TypeScript, `harneo-core` with `rusqlite`, the `claude` CLI at run time).

## Commands

```bash
cargo test -p harneo-core harnmap
npx vitest run src/features/harnmap src/features/profiles
npm run check:task   # before a ticket is reported done
npm run check:full   # before a batch merges into feat/harnmap
```

Tests never run the real `claude`: the runner takes a `ClaudeConfig` pointing at a fixture script (canned answers, a bad answer then a good one, two bad answers, a timeout, each failure kind), as `bundles` does.

## Project Structure

```
crates/harneo-core/src/harnmap/
  mod.rs        → MapView, category, edits, history, restore, staleness
  templates.rs  → the 8 categories
  map.rs        → MapDoc and its validation
  facts.rs      → facts and bundles from the files
  draw.rs       → prompt, answer schema, validation, retry, pins re-applied
  fit.rs        → Harnfit: prompt, verdict, quote checks, add to profile
  *_tests.rs
src/features/harnmap/
  HarnmapScreen.tsx, FirstDraw.tsx, MapCanvas.tsx, Node.tsx, Heartbeat.tsx,
  SidePanel.tsx, StagesPopover.tsx, HistoryPanel.tsx, RedrawDialog.tsx, ListView.tsx,
  HarnfitScreen.tsx, VerdictCard.tsx, states.tsx
  layout/ line.ts, cycle.ts, diamond.ts, parallel.ts, common.ts (+ tests)
```

## Code Style

```rust
/// Re-applies the user's pins and stage edits to a validated answer, whatever Claude returned.
fn keep_user_edits(answer: &mut MapDoc, current: &MapDoc) {
    for pin in current.placements.iter().filter(|p| p.pinned) {
        answer.placements.retain(|p| p.artifact != pin.artifact);
        answer.placements.push(pin.clone());
    }
    for stage in current.stages.iter().filter(|s| s.origin == StageOrigin::User) {
        answer.upsert_stage(stage.clone());
    }
}
```

```ts
/** Lays out a `line` map: stages left to right, stations stacked under each one. */
export function layoutLine(input: LayoutInput): Layout {
  const stageX = (i: number) => PAD + i * (STAGE_W + GAP);
  // ...pure: same input, same output; no DOM
}
```

## Testing Strategy

- **Templates:** all 8 parse, stage ids unique, loops point at real stages.
- **Draw:** prompt contains facts, pins and stage edits, never secret-flagged text; a valid answer is saved with model and duration; an invalid one is retried once, then `BadAnswer` with nothing saved; pins and user stages survive a re-draw that tried to move them; changes from the template computed; each failure kind maps to its state; cancel saves nothing.
- **Edits and history:** each `MapEdit`; old draws never change; restore copies; change lines between draws; staleness after an add, a removal, a changed digest and a category change.
- **Facts:** each fact kind from fixture artifacts, missing MCP servers.
- **Harnfit:** quotes found with file and line, an invented quote dropped; budget order (same kind first); latest-only save; staleness; add-to-profile makes a new version and a pinned placement; plugin and no-map refusals.
- **Export:** v2 round trip on a second data dir keeps draws (with and without history), fits, pins and notes, remapped by key; a v1 file imports; a malformed map part is refused; clone copies.
- **Layout:** each shape with 1, 7 and 12 stages and up to 10 stations per stage, no overlapping nodes, deterministic.
- **Frontend (mockIPC):** category picker and change; first draw, wait, replay, cancel; map editing, drag-to-zone with the popover, unpin, hide, list view, `i` and Escape on the heartbeat; history restore; re-draw dialog; every state card; Harnfit screen and its actions; library and import entry points; export toggles.

## Boundaries

- **Always:** validate Claude's answer before saving anything; re-apply pins and user stages; label facts and Claude's reading apart; keep old draws immutable; record the model on every draw and fit.
- **Ask first:** a new dependency (none planned); changes to the export format beyond v2.
- **Never:** send anything without the user's click; use Haiku; overwrite a draw; show map or note text as HTML; block an add because of a verdict.

## Success Criteria

1. A profile has a category, chosen at creation and changeable; a change sends nothing and marks the map out of date.
2. Draw places the profile's artifacts on the template's shape within the 300 s limit, or shows the right state; nothing half-drawn is ever saved.
3. Every edit autosaves; pins and user stage edits survive a re-draw; every earlier draw can be previewed and restored.
4. Facts and Claude's reading are visibly apart on the map, in the tooltip and in the side panel.
5. Harnfit gives a verdict with quotes that are all found in the files, and Add to profile places the artifact pinned.
6. Export → import on a fresh data dir brings the map, its history and the fits with no Claude call.
7. The screens match `design/harnmap/` in light and dark, at 1440 px and 960 px; the map stays smooth with 47 artifacts on Linux.
8. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review after the build: the suggestion chips written for the 7 non-software templates; the 4,000 / 12,000 / 120,000 character budgets; the 300 s timeout.
