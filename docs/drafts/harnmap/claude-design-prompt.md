# Prompt for Claude Design: Harneo Harnmap and Harnfit

> Paste everything below the line into Claude Design. Attach, if you can: `DESIGN.md`, `docs/screenshots/profile.png` and `docs/screenshots/library.png` (so it matches the app), and optionally `design/prototype/styles.css`.

---

Design a new feature for **Harneo**, a desktop app (Tauri, web UI in a desktop window) that manages a user's **harness for Claude Code**: the skills, commands, subagents, settings, MCP servers and other "artifacts" that shape how the AI agent works in a project. Users keep a **library** of artifacts, compose **profiles** from it (a named selection of artifacts), and install a profile into projects.

The feature is the **Harnmap**: an interactive diagram that shows a profile as a **factory**, a production line that takes work from an idea to a finished result, with each artifact placed where it acts. I want to see and click through the UX before any code is written. Build a clickable prototype.

**Names to use in the UI:** **Harnmap** is the diagram of a profile ("Heavy → Harnmap", "Draw the map", "Re-draw", "Map history"). **Harnfit** is the check of one new artifact against a Harnmap ("Harnfit in…" on a library card, a "Harnfit" button at import). In running copy, "the map" and "the fit" are fine.

## Who uses it and why

People who use Claude Code to get real work done, and not only developers: some build apps, others do research, writing, video production, social media or data analysis. They collect artifacts from public packs (Addy Osmani's agent-skills, Matt Pocock's skills) and write their own. Every day they find new skills and plugins online and wonder: *where would this go in my workflow, and would it clash with what I have?* The Harnmap lets them (1) see their profile as a factory, (2) shape it by hand, and (3) try a new artifact against it before adding it.

## Categories and Harnmap templates (design one for each)

A profile has a **category**, picked when the profile is created: one of the predefined categories below, or **Custom**, which the user describes in a few lines. The category can be changed later.

Each category has a **Harnmap template**: a starting skeleton of stages and a **shape** of flow that fits that kind of work. **This is where I need you most.** Each template must be beautiful *and* meaningful: real phases of that work, not decoration. Templates differ by **structure** (the shape of the flow), not by costume or theme: they all live in the same calm app. Refine this list: rename, merge or add categories, adjust stages and shapes. Tell me what you changed and why.

| Category | Shape | Default stages |
| :--- | :--- | :--- |
| Software development | A line with a loop back from Review to Build | Idea → Spec → Plan → Build → Test → Review → Ship |
| Research | An iterative loop: read and analyse until the question is answered | Question → Sources → Read & extract → Analyse → Synthesise → Verify → Report |
| Writing & content | A line with an editorial loop | Brief → Research → Outline → Draft → Edit → Fact-check → Publish |
| Video production | Pre-production, production, post-production, with parallel picture and sound tracks in post | Concept → Script → Storyboard → Assets → Edit ∥ Sound & captions → Publish |
| Social media | A cycle that fans out per platform | Plan → Ideate → Create → Adapt per platform → Schedule → Engage → Measure → back to Plan |
| Data & analytics | A line with a loop between cleaning and exploring | Question → Collect → Clean ⇄ Explore → Model → Visualise → Report |
| Design (UI, brand) | Diverge then converge (double diamond) | Brief → Research → Concepts → Prototype → Critique → Refine → Hand-off |
| Custom | Proposed by the model from the user's description | The model proposes, the user edits |

Show:
- **Category picker** in the "new profile" flow: a card per category with a small preview of its template's shape, plus Custom with a description field.
- **Every template's skeleton** (empty stages, before any artifact is placed), so the user sees the shape of their factory before anything is drawn. This is also the empty state of the Harnmap.

## The 10 artifact types (each has a tinted rounded-square icon)

Skill (blue, sparkles), Command (cyan, terminal), Subagent (violet, bot), Workflow (indigo, workflow), Rule (slate, scale), Settings (slate, sliders: one `settings.json` template holding permissions, hooks and status line), MCP server (orange, plug), Output style (pink, palette), Plugin (teal, package: a container of skills, agents and commands), CLAUDE.md (amber, file-text).

They are not all steps. In every category, lay the diagram out in **three layers**, readable at a glance:

- **The line**: stations along the production line, things invoked at a point in time. Skills, commands, subagents, workflows (a workflow may be drawn as a sub-line).
- **The toolbelt**: off the line, usable at any stage. Cross-cutting skills (e.g. Matt Pocock's productivity skills like `grill-me`), MCP servers (`github`, `sentry`), optionally wired to the stations that use them.
- **The floor**: always on, the environment the line runs in. CLAUDE.md, rules, the settings template (permissions as a fence; its hooks may appear as small gates on the line), output style.

## Screens and flows to design

### 1. Entry point
The existing profile builder (a page listing the profile's artifacts with a live folder tree; see the screenshot) shows the profile's category and gets a **Harnmap** button. It opens the Harnmap in a large near-full-window overlay. Every profile has its own Harnmap.

### 2. First draw: describe and draw
- Before anything is drawn: the category's **template skeleton**, with a **description** box ("How do you see this factory? Optional: leave it empty and Claude works it out from your artifacts") and a **Draw** button. Say in one line that the profile's artifacts go to Claude through the user's own `claude` CLI, and that it uses their plan and takes about a minute.
- A **drawing** state (20–60 s): make the wait feel alive and honest, e.g. stations lighting up on the skeleton as artifacts are placed, not a bare spinner.
- The **result**: the factory diagram. If the model renamed, added or removed stages from the template, it says so.

### 3. The diagram (the heart of it, give it the most care)
- The template's stages, in the template's shape. Each stage holds **stations**; each station is an **artifact node** (type icon, name, a one-line note from the model on what it does here). Links show hand-offs (e.g. `/build` → `test-driven-development` → `code-reviewer`). Something that "lights up" along the line to convey flow is welcome, but keep it calm.
- The toolbelt and the floor as distinct zones around the line.
- Click a node: a side detail shows the model's **reasoning** for the placement, the artifact's description, and an editable **note**.
- **The heartbeat dot.** Every artifact node has a small dot in its **bottom-right corner**:
  - **It beats:** a slow, soft pulse, like a heartbeat or a status light, so each node reads as a live part of the flow. **Colour: slate** (`#64748b`, dark `#8794a8`), because blue is reserved for selection. **While its tooltip is open, the dot turns blue** (`#2563eb`, dark `#3b82f6`) and stops pulsing, so only one dot is ever blue. Keep it calm: about one beat every 2 seconds, a slight scale and a fading ring, no glow. With `prefers-reduced-motion`, the dot stays still.
  - **Clicking the dot opens a small tooltip** next to it that sums up, in 2–4 short lines, **how this artifact takes part in the flow**: **Role** (what it does here, e.g. "Writes the failing test before any code"), **When** (what triggers it: invoked by `/build`, auto-triggered, called by `code-reviewer`…), **Hands off to** (the next artifact or stage), and the model's one-line note if it wrote one.
  - The tooltip is compact and terse, with artifact names in JetBrains Mono. It closes on Escape or a click outside, only one is open at a time, and it covers the links between nodes as little as possible.
  - The dot is separate from selecting the node: clicking the node opens the full side detail, clicking the dot only opens the quick summary.
  - Check it on a busy map (40+ nodes) so the pulsing doesn't turn into noise, and propose whether a Harnfit candidate (ghost node) gets a dot.
- **Bundles** (artifacts that only work together, e.g. a command + the skill it calls + its subagents) read as a group.
- Canvas controls: zoom, pan, fit to screen, minimap optional, **hide/show** per node, per type and per layer (to clean up a messy view).
- **Editing**, like a diagram tool: drag a node to another stage, remove it from the line (it goes back to "unplaced"), edit its note, and edit the **stages** themselves (rename, add, remove, reorder). Nodes the user placed or moved are **pinned** (a small pin mark) and survive a re-draw.
- **Re-draw**: edit the description and draw again. Before drawing, show which pinned nodes will stay.

### 4. Unplaced artifacts (right-hand panel)
- A tree: **Artifact type → artifact**, listing everything in the profile the model did not place. Make it clear this is normal ("Not everything is a step. These are available at any time.").
- **Drag and drop** an artifact from the tree onto a stage, the toolbelt or the floor; a small inline prompt then asks for a one-line note ("Why here?").
- Filter / search in the tree.

### 5. Harnfit: try one new artifact
- Two entry points: from a library artifact not in the profile ("Harnfit in…", pick a profile), and from the **import** review step, right after the user brings in something new.
- **One candidate at a time.** A **plugin is one candidate** but holds several parts: draw it as a group (plugin outline) whose parts (skills, commands, subagents…) are each placed and judged on their own, with one overall verdict for the plugin. Design this case carefully, it's the hard one.
- The existing factory is drawn as before, slightly receded; the candidate is drawn as clearly "new, not in the profile" (e.g. dashed outline, ghost style; don't spend the blue on it, blue is reserved for selection and the primary action).
- A **verdict** card attached to the candidate's proposed station: **Fits** (green), **Fits with changes** (amber, with the proposed adaptation, e.g. "replace the 'write tests after' step with a call to `test-driven-development`"), **Conflicts** (red, quoting the lines that clash and the artifact they clash with, drawn as a link between the two). Plus where it goes and why.
- Actions: **Add to profile** (primary), **Place elsewhere**, **Discard**. Make it obvious this is the model's opinion, not a rule.

### 6. Nothing is lost: saving, history, export
Every drawing costs the user tokens and time, so nothing may ever be lost. Make this visible and reassuring without being noisy:
- The Harnmap is **saved with the profile**, and every edit is saved as it happens (a quiet "Saved" indicator, no Save button).
- **History**: each draw and re-draw is kept (date, description used, what changed); the user can preview an earlier drawing and restore it. A re-draw never overwrites.
- **Harnfit results are saved** per profile and candidate; reopening one shows the saved verdict, with an option to run it again.
- The profile **export** includes the Harnmap and its history; show this in the export dialog. A profile **import** brings its Harnmap ready to use, with no tokens spent. **Cloning** a profile copies its Harnmap.

### 7. States
Empty profile (the template skeleton only); `claude` CLI not installed; "send artifacts to Claude" not turned on (off by default: explain and offer the setting); drawing failed (the previous drawing is still there); Harnmap **out of date** because the profile changed since it was drawn (show what changed, offer re-draw); category changed (re-draw on the new template, old drawing kept in history); a big messy profile (40+ artifacts), to prove hiding and zoom work.

## Sample data

**Software development**, profile **Heavy** (real names): skills `spec-driven-development`, `incremental-implementation`, `test-driven-development`, `code-review-and-quality`, `frontend-ui-engineering`, `security-and-hardening`, `webperf`, `grill-me`, `write-a-prd`; commands `/spec`, `/plan`, `/build`, `/review`, `/ship`; subagents `code-reviewer`, `test-engineer`, `security-auditor`; workflow `release`; MCP servers `github`, `sentry`; settings template `strict-guard` (deny rules, a path-guard hook, a status line); rule `rust-conventions`; output style `terse`; CLAUDE.md `rust-service CLAUDE.md`.

Harnfit candidates for Heavy: `tdd-lite` (a skill that says to write tests after the code: conflicts with `test-driven-development`), `commit-helper` (a command: fits at Ship), `db-migrator` (a subagent: fits with changes, it uses a `postgres` MCP server the profile doesn't have), and a plugin `agent-skills` (several skills and commands, some already in the profile: a mixed verdict).

**One non-software profile, fully drawn** (invent plausible artifact names, e.g. for Video production: skills `script-doctor`, `shot-list`, `caption-writer`, `thumbnail-brief`; command `/storyboard`; subagent `continuity-checker`; MCP servers `youtube`, `drive`; a CLAUDE.md for the channel's style guide).

## Visual direction (match the existing app)

- The category standard at full craft, benchmarked on **Vercel's dashboard and Linear**: white and slate neutrals, **one blue accent** (`#2563eb`, dark `#3b82f6`) used only for selection, focus and the primary action. Light and dark themes, following the OS.
- Green / amber / red are **state only** (`#15803d`, `#b45309`, `#b91c1c`): here, the fit verdicts. Never decoration, and never a category colour.
- Inter for the UI (13px base), JetBrains Mono for names, paths and counts. Hairline borders (`#e2e8f0`) over shadows; shadows only for floating layers. Lucide icons. Sentence-case, terse, technical copy, like a good CLI.
- The diagram and the category templates are where this feature earns its delight (the shapes, the line, the lights, the flow), but it must stay a working tool: calm, legible, dense, no neon, no glow.
- It's a desktop window: design for ~1440px wide, and check it holds at ~960px (half screen).

## What I want back

A clickable prototype covering the category picker, every template's skeleton, flows 2–6 with the sample data (Software development fully drawn, plus one non-software category fully drawn), and the states in 7. Where a choice is open (how hooks or plugins are drawn, the final category list, each template's shape), show your recommendation and note the alternative.
