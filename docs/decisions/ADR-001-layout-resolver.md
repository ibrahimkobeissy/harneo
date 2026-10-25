# ADR-001: Read a source's layout in three tiers; Claude only proposes

## Status
Accepted

## Date
2026-10-25

## Context
Import has to decide what a source holds before it can list candidates. People publish their work in many shapes: a plugin at the repo root, a marketplace pointing at `./` or at subfolders, packs under `skills/`, `.claude/skills/` or `.agents/skills/`, prompts mixed with docs. Review 1 (2026-10-25) showed the fixed scanner misreading real repos:
- `mattpocock/skills` stored with 191 files (its whole repo) as one plugin;
- `addyosmani/agent-skills` imported as two items (its plugin folder and a marketplace entry naming the same repo);
- no plugin enabled at scaffold, because none carried its marketplace.

The user proposed letting the `claude` CLI read each repo first and answer, in a fixed structure, what to take, so that import becomes mechanical.

Constraints:
- **Untrusted input.** The repo is untrusted, and if Claude decides what is imported, the repo's text can steer that decision.
- **Opt-in.** Sending content to Claude is opt-in and off by default (the one exception is audit phase 2), so import must work without Claude.
- **Can fail.** The CLI can fail: review 1's phase 2 runs all failed.
- **Same answer every time.** Updates and drift re-read the source and need the same answer each time.

## Decision
Resolve the layout in three tiers, in order (spec: `spec/library.md`, "Layout resolver"):
1. **Manifests** (`plugin.json` components, `marketplace.json` entries, `installed_plugins.json`) are authoritative and read mechanically.
2. **Known conventions** (project and pack folders, `.agents/skills`, loose `SKILL.md`) are read mechanically.
3. **Claude** is asked only about files the first two tiers left unplaced: per use with a button, or automatically when the `import.layoutWithClaude` setting is on (off by default). Its answer is a **proposal** in a fixed JSON schema. Every item is validated (path exists, stays inside the root, crosses no symlink, is unclaimed, and parses with its kind's parser) and audited as usual.

The resolved layout is stored with the source, and updates replay it without calling Claude. When a Claude call fails, the import pauses with **Retry**, **Continue with what was found** and **Cancel**.

## Alternatives Considered

### Claude first, for every source
- Pros: one path for every shape; no convention list to maintain.
- Cons:
  - Import would depend on an opt-in, fallible service.
  - The untrusted repo could steer what gets imported.
  - The answer can vary between runs, which breaks updates and drift.
  - It costs time and plan usage even for repos whose manifest already says exactly what they hold, as both review examples do.
- Rejected: the manifest is the author's own statement and is more reliable than Claude's reading of it.

### Fixed scanner only (status quo)
- Pros: deterministic, offline, free.
- Cons: every new publishing shape needs code; a repo the scanner misreads gives the user nothing to act on.
- Rejected: keep it as tiers 1 and 2, and add Claude for what it can't place.

### Claude decides, the user reviews every item
- Pros: the user is the safety net.
- Cons: users tick what is pre-ticked, and a long list of Claude-placed items is not a real review.
- Rejected in favour of mechanical validation plus the normal audit; the Choose step still labels each row with how it was placed.

## Consequences
- Plugins list, copy and audit only their components, which also removes audit noise from repo docs and tests.
- Claude can never place anything inside a manifest-claimed path, add files to an item, or affect the audit.
- A new stored field (the layout map, `layoutBy` per artifact) and an update state (*layout changed*).
- Repos with unusual layouts import only partly until the user asks Claude, or turns the setting on.
- Other agents' formats (Cursor rules, Codex `AGENTS.md`) stay out until a second adapter.
