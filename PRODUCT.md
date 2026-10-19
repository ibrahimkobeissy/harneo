# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(A Tauri 2 desktop app: the interface is web technology rendered in each OS's own engine, in a desktop window. WebKitGTK on Linux is the weakest of those engines and sets the ceiling.)

## Stack

React 19 + Vite + TypeScript inside Tauri 2, with a shared Rust core (`spec/app-shell.md`). The clickable prototype ([#7](https://github.com/ibrahimkobeissy/harneo/issues/7)) is plain static HTML/CSS/JS in `design/prototype/`, no build step.

## Users

**Designed for others first:** developers who find Harneo on GitHub, clone it and build it themselves. They work across many projects with Claude Code: solo developers, consultants, tech leads. They've collected skills, hooks, subagents and plugins from public packs (Matt Pocock's skills, pstack, Addy Osmani's agent-skills) and from their own work.

Their job: stop copying `.claude/` by hand into every repo, know where each artifact came from and whether it's outdated, unused or unsafe, and change a project's setup without fear of overwriting what it already has.

The maintainer is also a user, but first-time users are the design target: someone who has never heard of a "bundle", a "profile" or a "refit" must be able to understand them from the interface alone.

## Product Purpose

Harneo is a harness manager for Claude Code: it keeps every AI Artifact the user trusts in one library, composes profiles from it, and installs the right profile into any project, safely, with a backup, and with proof of what's inside.

Success: setting up a new repo's harness takes one click; refitting an existing one never loses a file; the user always knows what's installed where, where it came from and whether it's still used.

## Positioning

The only tool that holds a library of artifacts outside every project, ships it into projects as complete bundles (an artifact plus everything it depends on), and never loses a file doing it: every write is previewed, every replaced file is backed up, and removal restores the originals. It manages the whole harness (skills, commands, subagents, settings, MCP servers, output styles, workflows, plugins, CLAUDE.md, rules), not only skills.

## Operating Context

- **Occasional and task-driven.** Users open Harneo for a job, finish it and close it: import a pack, build or clone a profile, scaffold a new repo, refit an existing one, check drift, update an artifact, prune unused ones. Each visit should reach its task fast; it is not a dashboard kept open all day.
- Used next to a terminal and an editor. Artifacts are edited in the user's own editor, never inside Harneo.
- Reads Claude's session logs (`~/.claude/projects/`) for usage, the `claude` CLI for AI dependency detection, and git for imports and updates.
- A CLI companion (`harneo scaffold`, `harneo status`, `harneo update`) runs the same engine for terminals and scripts.

## Capabilities and Constraints

- The v1 features, the 10 artifact types and the user journey (Import → Library → Profiles → Scaffold / Refit → Live with it → Maintain and prune) are defined in `CONTEXT.md` §2–4. Terminology: AI Artifact, Library, Bundle, Profile, Scaffold, Refit, Project registry (`CONTEXT.md` glossary).
- **Writes are the dangerous moment.** Every write to a project is previewed first (full content for a scaffold, a content diff for a refit). Replacing always wins over merging, and every replaced file is backed up. Replacing `CLAUDE.md` or `settings.json` must be flagged in red when the project's version has lines the profile doesn't.
- No secrets stored: MCP secrets exist only as `${VAR}` placeholders.
- Never touches `CLAUDE.local.md`, `settings.local.json`, or anything outside the target project and Harneo's own data folder.
- Claude Code only in v1, designed so other agents can be added later.
- No in-app editor, no enable/disable switch, no marketplace browsing (`CONTEXT.md` §4).
- Distribution is source only; built and tested on Linux by the maintainer.

## Brand Commitments

- **Name:** Harneo. No logo or visual identity exists yet.
- **Voice: terse and technical**, like a good CLI. Exact file paths, short labels, no hand-holding. Warnings are precise and quantified (e.g. "settings.json: 14 lines will be lost").
- Free and open source (MIT). Not for sale.

## Evidence on Hand

- No users, testimonials, screenshots, metrics or benchmarks exist yet; none may be invented.
- Real public artifact packs exist and may appear as realistic sample data in the prototype: Matt Pocock's skills, pstack, Addy Osmani's agent-skills.
- Competitors and references are documented in `CONTEXT.md` §7 (HarnessKit, Loadout, tackroom, Moltenbase).

## Product Principles

1. **Show before writing.** Nothing touches a project until the user has seen exactly what will change.
2. **Never lose a file.** Backups before every write, restore on removal, never a silent overwrite, upstream updates never clobber local edits.
3. **Proof over trust.** Every artifact shows its source, version, trust score, what it can reach and whether it's used.
4. **Terse for the expert, clear for the newcomer.** Labels stay short and technical; each concept (bundle, profile, refit, drift) is explained the first time a user meets it, then gets out of the way.
5. **Whole units only.** Picking an artifact pulls in its whole bundle; plugins stay whole; settings is one template. Nothing ships half-wired.

## Accessibility & Inclusion

No product-specific requirement was established. (An accessibility gate was considered for `CONSTRAINTS.md` and deliberately not enforced.)
