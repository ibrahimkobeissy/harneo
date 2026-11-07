# Harnmap design review: what needs a change

> Review of the Claude Design prototype (2026-11-07), ticket #224. Only the items that **can't be built as drawn** are listed: everything else in the design is buildable as is.
>
> ⚠️ buildable with a change · ❌ not buildable as drawn · ❓ needs a spike (a small test build) first · 🟦 your decision

## The 8 that matter most

1. **Automatic layout per shape** (B1)
2. **Facts vs Claude's opinion** (A1)
3. **Spike for live placement** (C1)
4. **Hooks drawn along the whole line, not as a gate before Build** (B5)
5. **Export inside the existing `.harneo-profile.json`** (G1)
6. **Consent: no switch before Draw** (H1, your decision)
7. **Quotes checked word for word** (F1)
8. **Animation performance on WebKitGTK** (B3)

---

## A · Across the whole design

**A1 ⚠️ Facts vs Claude's opinion.** Mark everywhere what Harneo knows from the files and what Claude only thinks. This is the most important principle the design is missing.
- Claude Code has no pipeline: nothing makes `/review` send work back to Build, so most links and every "Hands off to" are Claude's reading.
- Some links are facts Harneo already knows: bundles (a command calls a skill), a subagent's `skills` and `mcpServers` fields, a skill's allowed `mcp__…` tools, a skill's manual-only flag, a rule's paths.
- Proposal: **solid link = known from the files**, **dashed link = Claude's reading**. Same split in the heartbeat tooltip, the side detail and the wires.

**A2 ⚠️ Claude's answer format.** Claude returns: stages (id, name, changed or not), placements (artifact, layer, stage, order, note, why, role, when, hands-off), links and loops, and unplaced artifacts. Harneo validates the answer against a schema (`--json-schema`, as today): every artifact exists, each is placed once, every stage is valid. Anything invalid goes to Unplaced. Everything Harneo can compute (stages kept or renamed, what's unplaced, changes between draws, history lines) is computed by Harneo, never asked of Claude.

**A3 ⚠️ Show the model.** The design never shows which model drew a map. Add "Opus · 52 s" to the draw summary and to each history entry.

**A4 ⚠️ Add a list view** of the map: a table of artifact, layer, stage and note. It's cheap, works with screen readers, and helps with big profiles.

**A5 🟦 Proposed v1 cut.**
- **v1:** templates, draw (no live placement unless the C1 spike succeeds), the map with editing, pins and history, Harnfit for a single artifact.
- **v1.1:** plugin Harnfit, wires, social media lanes, live placement, the full factory decoration.

## B · The Harnmap

**B1 ❌ The layout is placed by hand.** Every node, stage and label sits at fixed pixel positions. A real profile has any number of artifacts per stage, 5–12 stages (some renamed or added), long names, and different shapes. We need an **automatic layout for each shape** (line, line + loop, cycle, double diamond, parallel tracks) that puts the stages along the shape and stacks stations inside each. It's the biggest engineering item. Recommendation: React Flow (`@xyflow/react`, MIT) for pan, zoom, drag, selection and minimap, with our own layout function per shape.

**B2 ⚠️ Factory decoration** (intake, chutes, valves, machines, lights, beams) must be **generated from the layout**, not drawn per board. If v1 is short on time: keep stage blocks, lights and the beam on the main line, and cut chutes and valves.

**B3 ⚠️ Animation performance on WebKitGTK.** Beams on every pipe plus a heartbeat and ring on 24–47 nodes would run continuously, on the weakest engine we support. Use one shared animation clock, animate only transform and opacity (no `stroke-dasharray` beams on many paths), pause when the window is hidden, and test 47 nodes on Linux.

**B4 ⚠️ Simplified view at low zoom.** The 47-artifact state already draws simple columns instead of the factory. Make that an explicit rule: below about 75% zoom, simple boxes and no decoration. It also solves B3.

**B5 ⚠️ A hook is not a gate before Build.** A `PreToolUse` path-guard in `settings.json` fires on **every** matching tool call, at every stage. Only a hook in a skill's own frontmatter is scoped to that skill. Draw settings hooks along the **whole line** (or on the fence), labelled with their event and matcher (e.g. `PreToolUse · Write|Edit`), which Harneo can read from `settings.json`. Skill-scoped hooks attach to their skill.

**B6 ⚠️ Copy that implies enforcement.** "Review sends work back to Build: the loop is live" suggests Harneo enforces the flow. Reword it, e.g. "the loop is drawn".

**B7 ⚠️ No way to unpin.** Moving a node pins it, but nothing unpins it. Add "Unpin, let Claude place it". Also define what happens to pinned stations when their stage is removed: they go to Unplaced, but do they stay pinned?

**B8 ⚠️ Re-draw with pins.** Claude gets the pins and stage edits as constraints, and Harneo **re-applies them whatever Claude returns**. Stages are stored by id, not by name, so renames survive a re-draw.

**B9 ⚠️ Stage editor on shaped templates.** Define the rules: what does adding a stage to a double diamond mean? Loops attach to stage ids, and removing a stage removes its loops. The layout must accept any number of stages on every shape.

**B10 ⚠️ Heartbeat dot.**
- It's a **second tab stop per node** (48 for Heavy). Consider opening the tooltip with a key on the focused node (e.g. `i`) instead.
- The **Video board has no dots**, so it doesn't match.

## C · First draw

**C1 ❓ Live placement.** The design shows artifacts riding the line *while Claude is still working* (READING, then PLACING, "12 / 22 placed", a timestamped log). Today Harneo gets **one answer at the end** from `claude -p --output-format json`. Live placement would need `--output-format stream-json`, an answer that gives one placement at a time, and parsing it as it arrives: plausible, but untested with schema-validated output. Ask Design for the **fallback screen** too: an honest wait ("Reading 24 artifacts · 00:41") that replays the placements in 2–3 s once the answer lands.

**C2 ⚠️ Suggestion chips are software-only.** "+ TDD inside Build" means nothing for video. Define them per template.

**C3 ⚠️ "About a minute" is optimistic.** On Opus with a large profile it can take several minutes. Say "usually 1–3 min", or base it on the profile's size.

## D · Profile builder and category

**D1 ⚠️ Changing the category re-draws straight away** ("The map is re-drawn on the template"), spending tokens without a click. It should only mark the map out of date; the user then clicks Re-draw, as the "Category changed" state already shows. Make the two consistent.

**D2 🟦 Two ways of saving on one page.** The profile builder keeps an explicit Save (profile versions) while the map saves automatically. Does editing the map bump the profile version? My suggestion: no. The map belongs to the profile but isn't installed into projects.

## E · Templates

**E1 🟦 Design changed the templates. Validate:**
- **Video production:** "Assets" became "Shoot & assets".
- **Design:** adds a **Define** stage and merges Critique and Refine into a Prototype ⇄ Critique loop.
- **Research:** adds a Verify → Sources loop.
- **Data & analytics:** adds a Model → Explore loop.
- **Social media:** gets platform **lanes**, which Claude creates from your artifacts.

**E2 🟦 Social media lanes add a new concept** (sub-stages created per profile) to the data model and the layout. Keep them for v1.1, or treat them as normal stages grouped under "Adapt".

**E3 ⚠️ Custom picks from known shapes.** For Custom, Claude chooses one of our shapes and only names the stages. It never invents a geometry, because we can only lay out shapes we've built.

## F · Harnfit

**F1 ⚠️ Quotes must be checked.** Verdicts quote "SKILL.md:12". Models misquote: Harneo must check every quote word for word against the file and compute the line number itself, and drop or flag any quote that doesn't match.

**F2 ⚠️ Quoting your existing artifacts needs their full text**, so "send only descriptions" (in `idea.md`) doesn't hold. Proposal: send the saved map, the candidate in full, and the full text of the artifacts on the candidate's likely stage and of the same type, picked by Harneo from the saved map. Alternative: send everything (slower).

**F3 ⚠️ "Add with postgres"** only works if a postgres MCP server is in the library. Otherwise the button becomes "Import postgres first" or "Add without it". Adding an MCP server also goes through the `${VAR}` secrets flow.

**F4 ⚠️ "v1.4 → v1.6" on plugin parts and in the out-of-date list.** Skills rarely have version numbers. Show "same as yours" or "differs from yours" (by content hash), with a diff.

**F5 ⚠️ "Add plugin" when parts are already in the profile** creates duplicates (the same skill twice), so a name clash in the project. Warn about the overlap before adding.

**F6 ⚠️ Harnfit needs a map.** Missing state for a profile with no map yet: "Draw Heavy's map first", or offer to draw both in one run (more tokens).

**F7 ⚠️ Harnfit from import.** The candidate isn't in the library yet; its content comes from the import scan. A saved fit stores the source, commit and content hash, and shows **out of date** when the candidate or the profile changes.

**F8 ⚠️ The import board shows a verdict already there**, so Harnfit ran automatically at import. That sends data without a click: it needs an opt-in setting, off by default (like bundles at import). Otherwise it's one Harnfit button per artifact.

**F9 ⚠️ "About 20 s"** is too optimistic on Opus with full texts. Expect 30–90 s.

**F10 🟦 Fit history.** Does a fit keep a history like the map, or only the latest result?

## G · Export and import

**G1 ❌ `heavy.harneo.zip` doesn't exist.** Harneo exports one JSON file, `<name>.harneo-profile.json` (format v1, 50 MB cap, untrusted on import). The map, its history and saved fits become new fields in it, with **formatVersion 2**. Fix the mock.

**G2 ⚠️ Artifact ids change on import.** The map references artifacts by a stable key (type, name and content hash) and remaps them on import. The map in the file is untrusted: check its size and schema, and show notes only as plain text.

## H · States

**H1 🟦 No switch before Draw.** "Sending artifacts is off" blocks Draw until a switch is turned on. The app's consent rule (`spec/bundles.md`, decision 2) is: **automatic** sending is off by default, and an **explicit action with a clear disclosure** counts as consent. Draw already discloses what's sent. Recommendation: no switch for Draw, Re-draw or Run again; keep the setting only for automatic Harnfit at import (F8). Keep the "What is sent / Never" text in the Draw panel.

**H2 ⚠️ CLI not found.** Don't hardcode `npm install -g @anthropic-ai/claude-code`. Link to the official Claude Code setup page and reuse the app's existing wording for a missing CLI.

**H3 ⚠️ Missing states:**
- `claude` not signed in
- model not available (offer Sonnet)
- timeout
- Claude's answer didn't validate (retry once, then an error)
- a profile with nothing on the line (only floor and toolbelt)
- Harnfit with no map (F6)
- a fit that's out of date (F7)

---

## Second pass (2026-11-07, design version 1794069384)

**Done in the design:** A1, A3, A4, B4, B5, B6, B7, B8, B10, C1 (the replay fallback is the v1 default; live is a toggle), C3, D1, E3, F1, F3–F10, G1, G2, H1, H2, H3. Decided in the design's canvas note: A5, D2, E1, E2, F10, H1.

**Left for the spec, not the design** (engineering work the mock can't show):
- **B1** automatic layout per shape, **B2** decoration generated from the layout, **B3** animation performance (beams above 75% zoom still animate `stroke-dashoffset`: test on WebKitGTK, pause when hidden).
- **B9** what adding a stage means on a double diamond or parallel tracks (the removal and rename rules are now in the design).
- **F2** which full texts Harnfit sends.
- **A2** the answer schema and checks.
- **Timeout:** the design says 5 min. Confirm in the spec.

**Three small copy fixes:**
1. `strict-guard` still says "Gate + permissions" on its node and "the path-guard gate" in its tooltip. The hook is drawn along the whole line now: use "Fence + hook".
2. "Model not available" says Sonnet "usually places artifacts the same way". We can't back that claim: keep only "the history entry says which model drew it".
3. The draw suggestions show the Software development set only. Fine for the mock; the spec defines one set per template.
