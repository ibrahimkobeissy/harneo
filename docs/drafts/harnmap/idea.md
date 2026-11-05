# Harnmap: see your factory before you build it

> **Status: draft, UX first.** Ticket [#224](https://github.com/ibrahimkobeissy/harneo/issues/224), branch `feat/harnmap`. Nothing gets built until the UX is validated in Claude Design (`claude-design-prompt.md`). After that it gets a `CONTEXT.md` update, a capability entry, a spec (`spec/harnmap.md`) and tickets, as usual.
>
> **Names (decided 2026-11-05):** **Harnmap** is the diagram of a profile, **Harnfit** is the check of a new artifact against it. In running text, "the map" and "the fit". They double as CLI verbs: `harneo map <profile>`, `harneo fit <artifact> --profile <profile>`.

## Why

Harneo manages the harness: library, profiles, scaffold, refit. Many users build a real **factory** with it, a workflow that takes work from an idea to a finished result. That isn't always software: some people use Claude Code for research, writing, video production or social media. Every day you come across new skills, commands and plugins online that you want to try, and the question is always the same: *where would this go in my factory, and how would it get along with what's already there?*

Today you only find out after you install it. The Harnmap answers before you do.

## Categories and Harnmap templates

Not everyone builds software, so a profile has a **category**. The user picks it when creating the profile: one of the predefined categories, or **Custom**, which they describe in a few lines.

Each category comes with a **Harnmap template**: a starting skeleton of stages, and a **shape** of flow that suits that kind of work. Templates must be meaningful, real phases of that work, and beautiful. They differ by **structure** (the shape of the flow), not by costume. The model fills the template with the profile's artifacts, and may rename, add or remove stages when the profile calls for it (it says so). The user can edit the stages too.

Proposed categories (to be refined in Claude Design):

| Category | Shape | Default stages |
| :--- | :--- | :--- |
| **Software development** | A line with a loop back from Review to Build | Idea → Spec → Plan → Build → Test → Review → Ship |
| **Research** | An iterative loop: read and analyse until the question is answered | Question → Sources → Read & extract → Analyse → Synthesise → Verify → Report |
| **Writing & content** (articles, docs, newsletters) | A line with an editorial loop | Brief → Research → Outline → Draft → Edit → Fact-check → Publish |
| **Video production** | Three phases, with parallel picture and sound tracks in post-production | Concept → Script → Storyboard → Assets → Edit ∥ Sound & captions → Publish |
| **Social media** | A cycle that fans out per platform | Plan → Ideate → Create → Adapt per platform → Schedule → Engage → Measure → back to Plan |
| **Data & analytics** | A line with a loop between cleaning and exploring | Question → Collect → Clean ⇄ Explore → Model → Visualise → Report |
| **Design** (UI, brand) | Diverge then converge (double diamond) | Brief → Research → Concepts → Prototype → Critique → Refine → Hand-off |
| **Custom** | Proposed by the model from the user's description | Proposed by the model; the user edits them |

The category can be changed later. That re-draws on the new template, and the previous drawing stays in the history.

## The feature in three parts

### 1. Draw the profile

- From any profile, a **Harnmap** button opens a large window. Every profile has its own Harnmap.
- The user can write a **description** of how they see the factory ("spec first, then TDD build, review with two agents, ship through /ship"). It is optional: with no description, the model works it out from the category template and the artifacts.
- Harneo sends the profile's artifacts, the category template and the description to Claude, which lays them out: which artifact sits at which stage, how they hand off to each other, and why.
- Harneo draws the result as an **interactive diagram**, not an image: a production line with stations (nodes, lights) and the artifacts placed on them.
- The user can change the description at any time and **re-draw**.

### 2. Not everything belongs on the line, and the user has the last word

- The model **must not force** every artifact onto the line. Many are not steps: Matt Pocock's skills, for instance, are mostly productivity tools usable at any phase. Leaving them off the line is correct, not a failure.
- On the right, a **tree of unplaced artifacts** (Artifact type → Artifact). The user drags one onto the diagram and writes a short note on why it goes there.
- The user can **change what the model did**: move an artifact, remove it from the line, edit its note, and edit the stages (rename, add, remove, reorder).
- Diagram-tool basics: zoom, pan, **hide** items to clear a messy view, select, move. Think draw.io, but the content is the harness.
- Anything the user placed or moved is **pinned** and survives a re-draw.

### 3. Harnfit: try a new artifact before adding it

- Two entry points: from a library artifact ("Harnfit in…", pick a profile), and from the **import** review step.
- **One candidate at a time.** A **plugin counts as one candidate**: it is drawn with its parts (skills, commands, subagents…), each part placed and judged on its own, grouped by a plugin outline, with one overall verdict for the plugin. No bulk check of a whole pack.
- The Harnmap shows the existing factory as before, plus the **candidate drawn differently** ("new, not in the profile").
- The model says **where it goes** (which stage) and **what it thinks**: it **fits**; it fits **with changes** (the suggested adaptation); or it **conflicts** with the existing workflow, quoting the lines that clash (e.g. "write tests after" in a TDD-first factory).
- The user then adds it to the profile, places it elsewhere, or drops it.

## What gets drawn: three layers (agreed 2026-11-05)

Not all 10 artifact types are steps. The layers hold for every category.

| Layer | What it means | Types |
| :--- | :--- | :--- |
| **The line** (stations) | Invoked at a point of the workflow: a step, a gate, a hand-off | Skills, Commands, Subagents, Workflows (a workflow can be drawn as a sub-line of its own) |
| **The toolbelt** (off the line, anytime) | Available at any stage, used on demand | Cross-cutting skills (e.g. productivity skills), MCP servers (tools that stations use; can be wired to them) |
| **The floor** (always on, the environment the line runs in) | Ambient: shapes every step without being one | CLAUDE.md, Rules (path-gated rules could be zones), Settings (permissions as the fence), Output styles |

Edge cases for Claude Design to propose:
- **Hooks** live inside Settings (one artifact on purpose, `CONTEXT.md` §2) but fire at events (`PreToolUse`, `Stop`…). They could be drawn as small gates on the line, owned by the Settings template.
- **Plugins** are containers. Their parts are drawn individually, grouped by a plugin outline.
- **Auto-invoked skills** can fire anywhere: on the line or in the toolbelt, as the model judges, and the user can override.

## Nothing is lost

Every drawing costs the user tokens (their Claude plan) and a minute or so of waiting, so it is kept like a file:
- The Harnmap is **saved with the profile**, and every user edit is saved as it happens (placements, notes, hidden items, stages, pins).
- A **re-draw never overwrites**: the previous drawing stays in a history, and the user can go back to it.
- **Harnfit results are saved** too (per profile and candidate), so reopening one does not spend tokens again.
- The Harnmap is **in the profile export**. Importing a profile brings its Harnmap, with no tokens needed. **Cloning** a profile copies its Harnmap.
- When the profile changes (an artifact added or removed), the Harnmap shows as **out of date**, with what changed. It is not re-drawn silently.

## Decisions (2026-11-05)

1. Three layers (line, toolbelt, floor) as above.
2. Stages are not fixed: they come from the profile's category template, adjusted by the model and the user.
3. One Harnmap per profile.
4. Harnfit from both the library and import.
5. One candidate at a time; a plugin is one candidate with its parts. No bulk pack check.
6. The Harnmap and every edit are saved, kept through re-draws, exported and imported with the profile.

## Notes for the spec

- **The model returns data, not a drawing**: a structured layout (stages, placements, links, notes, unplaced items, verdicts) that Harneo renders. That is what makes it interactive, lets a re-draw keep pinned edits, and keeps it diffable and exportable.
- **It is the "judgement" half of the harness check** (`CONTEXT.md` §6). The rule-based half (name clashes, missing bundle members) can show on the same diagram as warnings, at no cost.
- **Advice, not truth.** Verdicts are non-deterministic: show them as the model's opinion with its reasons and quotes, and never block anything.
- **Sending data is opt-in**: through the user's `claude` CLI and plan, behind a setting that is off by default.
- **Size**: send frontmatter and descriptions for the whole profile, and full text only for the candidate in a Harnfit.

## Still open

- The final list of categories, and each template's stages and shape (Claude Design proposes, the user validates).
- Whether a category also carries a recommended starter set of library artifacts (a later idea, not v1 of this feature).
