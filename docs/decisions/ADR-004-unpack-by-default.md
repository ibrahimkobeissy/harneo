# ADR-004: Unpack plugins by default; whole only when they must be

## Status
Accepted. Changes the default of ADR-002 (whole → unpacked); ADR-002's rules on what can be picked, exclusivity and the stored choice still hold.

## Date
2026-11-07

## Context
ADR-002 let the user unpack a plugin into its parts but kept **whole** as the default. Two problems came up while building the Harnmap (`spec/harnmap.md`):

- **Durability.** A whole plugin installs from its marketplace (`claude plugin install`, `spec/install-engine.md` 4b). Harneo keeps a copy of its files, but if the maintainer deletes or closes the repo, no new scaffold or refit can install it. Unpacked parts are ordinary library files, installed by copying: they survive the upstream disappearing.
- **Exceptions in the code.** A whole plugin is one opaque item, so every feature that needs to see what it holds opens it on its own terms. About 23 core files and 10 app files special-case plugins today (audit, usage, drift, install, layout, import, Harnmap, Harnfit), and each new feature adds more. On the Harnmap a pack like `addyosmani/agent-skills` shows up as one box on the toolbelt instead of its skills on their stages.

Most plugins users import are packs of skills, commands, agents, MCP servers and output styles, which work the same as loose files. Some plugins only make sense whole: their value is in parts Claude Code wires only inside a plugin (hooks, monitors, LSP servers, mods, `bin/` tools), or their parts refer to each other through `${CLAUDE_PLUGIN_ROOT}`.

## Decision
- **Unpacked is the default at import.** A plugin's pickable parts (ADR-002: skills, agents, commands, output styles, MCP servers) become ordinary library artifacts with `fromPlugin`, all ticked unless the user unticks them.
- **Whole only when it must be.** Harneo proposes whole, and says why, when the plugin holds a part that only works inside a plugin (hooks, monitors, LSP servers, mods, `bin/` tools, themes, workflows, its own `settings.json`), when a part refers outside itself through `${CLAUDE_PLUGIN_ROOT}`, or when it is a remote-only marketplace entry. For a plugin with both kinds of parts, the user may still unpack the pickable ones and is told which parts are lost.
- **One-click Unpack** for plugins already in the library whole: they become their parts, and every profile that holds the plugin gets its parts instead, as a new profile version. A plugin that must stay whole has no Unpack button, only the reason.
- **One place for plugin handling.** A single `plugin` module in `harneo-core` answers what a plugin holds and whether it must stay whole; the other modules call it instead of special-casing plugins.
- **No local marketplace.** Harneo does not serve its own copy of a whole plugin as a marketplace: the project goes to git, and a teammate who clones it would not have that local folder (user decision, 2026-11-07).

## Alternatives Considered

### Keep whole as the default (ADR-002 as is)
- Pros: matches how plugins are published; one item, one install; updates from the marketplace.
- Cons: the plugin dies with its repo; every feature keeps special-casing plugins; skill packs are opaque on the Harnmap and in Harnfit.
- Rejected by the user (2026-11-07).

### Treat a plugin as a container (install whole, read its members)
- Pros: one read path for every feature; installs stay faithful to the plugin.
- Cons: keeps the durability problem for the common case of skill packs; adds a parent–member model to maintain.
- Rejected: unpacking by default solves both problems for skill packs, and whole plugins become rare.

### Always unpack, drop whole plugins
- Pros: the simplest code.
- Cons: plugins built on hooks, monitors, LSP servers, mods or `bin/` tools could no longer be imported.
- Rejected: those plugins only work whole.

### Serve Harneo's copy as a local marketplace
- Pros: whole plugins survive their repo.
- Cons: works only on the machine that has the folder; a cloned project would not find it.
- Rejected by the user (2026-11-07).

## Consequences
- The import Choose step switches its default and shows why a plugin stays whole; the scan reports "must stay whole" with its reason.
- A library action to unpack a whole plugin, which rewrites the profiles that hold it as new versions.
- Plugin handling moves into one core module; the special cases in audit, usage, drift, install, layout, Harnmap and Harnfit go through it.
- Whole plugins remain subject to their marketplace: if it disappears, they can't be installed in new projects. Accepted for the plugins that must stay whole.
- Unpacked skills lose the `plugin:` namespace, as in ADR-002; the duplicate-name check covers clashes.
