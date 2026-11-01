# ADR-002: Import a plugin whole or pick its parts, never both

## Status
Accepted. Amends the "kept whole, never unpacked" rule of `CONTEXT.md` §2 and §4 and of ADR-001 (a plugin's files are still its manifest and components).

## Date
2026-11-01

## Context
Until now a plugin was one library item, enabled at scaffold through `settings.json` and installed from its marketplace (`spec/install-engine.md` 4b). Many packs are published as plugins (`addyosmani/agent-skills`, Impeccable), and users want a few of their skills, not all of them: every skill a plugin brings adds to each session's context, and a profile can't leave any of them out.

Constraints:
- **Claude Code enables whole plugins.** There is no way to turn on only some parts of a plugin, so picking parts means copying them out as ordinary artifacts.
- **Some parts only work inside a plugin.** Hooks, monitors, LSP servers, `bin/` tools, themes, workflows and the plugin's `settings.json` are wired by the plugin system, and anything referring to `${CLAUDE_PLUGIN_ROOT}` breaks once moved.
- **Names change.** Inside a plugin a skill is `plugin:skill`; copied out it is `skill`, and can clash with another.
- **Updates must still work.** A copied part has to keep track of the plugin's repo, subfolder and commit.
- **The layout resolver stays mechanical.** What a plugin holds comes from its manifest (tier 1, ADR-001), so the parts are known without Claude.

## Decision
At import, each plugin is taken **either whole** (the default, as before) **or unpacked**, never both:
- **Pickable parts:** skills, agents, commands, output styles, and each MCP server the plugin declares. They come from the components the manifest declares (or Claude Code's default folders), become ordinary library artifacts of their kind, and carry `fromPlugin` (plugin, folder, marketplace).
- **Only with the whole plugin:** hooks, monitors, LSP servers, `bin/`, themes, workflows and the plugin's `settings.json`.
- **A part that refers outside itself** (`${CLAUDE_PLUGIN_ROOT}` pointing outside its own files) is listed but can't be picked, and the reason is shown.
- **Exclusive per plugin**, in one import and across the library: no parts of a plugin while it is in the library whole, and the other way round.
- **The choice is stored on the source** and replayed by update checks, like the layout (ADR-001).
- A remote-only marketplace entry (another repo, never fetched) can only be taken whole.

## Alternatives Considered

### Keep plugins whole (status quo)
- Pros: matches Claude Code; one item, one install; updates come from the marketplace.
- Cons: all or nothing; the user's context fills with skills they never use.
- Rejected: it's the problem users asked to solve.

### Allow whole and parts together
- Pros: flexible.
- Cons: the same skill loads twice, once as `plugin:skill` and once as `skill`; drift and updates get two owners for one file.
- Rejected by the user (2026-11-01).

### Make hooks pickable too
- Pros: complete.
- Cons: a plugin's hooks would have to be merged into a Settings template, and Harneo writes `settings.json` whole from one template (`CONTEXT.md` §5).
- Rejected by the user (2026-11-01) as too complex for the gain.

## Consequences
- A new scan option (plugins to unpack), a stored choice per source (a migration), and an exclusivity check at import.
- Unpacked parts install like any artifact of their kind and are audited, bundled, tagged and tracked for usage like them; bundles can now link parts of what was a plugin.
- Unpacked skills lose the `plugin:` namespace; the existing duplicate-name check covers clashes.
- A plugin whose parts depend on each other through `${CLAUDE_PLUGIN_ROOT}` can only be taken whole.
