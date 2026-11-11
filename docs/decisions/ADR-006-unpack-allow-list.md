# ADR-006: A plugin is unpacked only when every part is known to work as a copy

## Status
Accepted (user decision 2026-11-11). Changes ADR-004's "For a plugin with both kinds of parts, the user may still unpack the pickable ones and is told which parts are lost": such a plugin is now whole only. ADR-004's default (unpacked) and the rest of ADR-002 and ADR-004 still hold.

## Date
2026-11-11

## Context
ADR-004 unpacks plugins by default and keeps one whole only when a check finds a reason: a whole-only part (hooks, monitors, LSP servers, mods, `bin/`, themes, workflows, `settings.json`), a part reaching outside itself through `${CLAUDE_PLUGIN_ROOT}`, or a remote-only entry. Anything those checks do not know about is unpacked.

Importing `pbakaus/impeccable` (2026-11-11) showed both limits of that rule:

- **A mixed plugin could still be unpacked.** Its plugin holds one skill, four agents and three hooks (SessionStart, PostToolUse on `Edit|Write`, Stop) that run the skill's own `scripts/impeccable`. The user picked its parts; the library got the skill and three agents, without the hooks and without `impeccable-asset-producer` (refused, it calls `${CLAUDE_PLUGIN_ROOT}/skills/impeccable/scripts/impeccable`). A project scaffolded from those parts is not what installing the plugin gives, while the library shows them as that plugin's parts.
- **The checks list what forbids unpacking, so every unknown case unpacks.** Reading the plugin reference (code.claude.com/docs/en/plugins-reference, 2026-11-11) found features the checks miss:
  - `${CLAUDE_PLUGIN_DATA}` (the plugin's persistent data folder) and `${user_config.KEY}` (values asked when the plugin is enabled) resolve only inside a plugin.
  - `${CLAUDE_PLUGIN_ROOT}` is accepted today when it points inside the part's own files, yet the install copies the part into `.claude/` without rewriting it, where the variable is empty.
  - Parts call each other by their namespaced names (`/<plugin>:<skill>`, `<plugin>:<agent>`, `mcp__plugin_<plugin>_<server>__<tool>`); unpacked parts lose the prefix and those calls point at nothing.
  - Plugin agents ignore `hooks`, `mcpServers` and `permissionMode`; the same agent copied into `.claude/agents/` runs them.
  - `dependencies` (other plugins it needs), `userConfig`, `channels`, `types` (mods) and the experimental keys only mean something inside a plugin.

Harneo is open source and meets repos nobody has tested. Each new case so far has been one more check added after a user found it (ADR-005 for `mattpocock/skills`, this one for impeccable). A list of what forbids unpacking will keep missing cases; a list of what allows it fails safe.

Unpacking matters mostly for the Harnmap, which places parts on stages: a whole plugin is one box there.

## Decision
- **Allow-list, not deny-list.** A plugin is unpacked, by default or on request, only when all of these hold:
  1. Its components are only skills, agents, commands, output styles and MCP servers, from the default folders or from the manifest keys `skills`, `agents`, `commands`, `outputStyles`, `mcpServers`.
  2. Its manifest has no key beyond those and the metadata Claude Code does not act on (`name`, `displayName`, `version`, `description`, `author`, `homepage`, `repository`, `license`, `keywords`, `metadata`, `icon`, `$schema`, the listing URLs, `defaultEnabled`). Any other key (`hooks`, `lspServers`, `workflows`, `experimental`, `themes`, `monitors`, `settings`, `userConfig`, `dependencies`, `channels`, `types`, or one Harneo does not know) keeps it whole.
  3. It has none of `hooks/hooks.json`, `monitors/`, `.lsp.json`, `bin/`, `themes/`, `workflows/` or a root `settings.json`.
  4. No part's files use `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}` or `${user_config.…}`, wherever they point.
  5. No part names another part of the same plugin by its namespaced name (`<plugin>:<name>`, `/<plugin>:<name>`, `mcp__plugin_<plugin>_…`).
  6. No agent sets `hooks`, `mcpServers` or `permissionMode` (inert in a plugin, live in `.claude/agents/`).
- **Whole means whole.** A plugin that fails any rule has no Pick parts and no Unpack button; the import and the library show the first rule it fails, in words. It installs from its marketplace (`claude plugin install`, `spec/install-engine.md` 4b), so a scaffolded project gets exactly what installing the plugin by hand gives.
- **Parts are all or nothing.** An unpackable plugin may still be imported with some parts unticked (ADR-002), but a part is never refused on its own: rule 4 or 5 failing on one part keeps the whole plugin whole.

## Alternatives Considered

### Keep ADR-004's deny-list and add the missing checks
- Pros: smallest change; plugins already unpacked stay valid.
- Cons: the next untested repo brings the next missing check; the user found this one by testing their own imports.
- Rejected: a deny-list fails open on unknown cases.

### Keep unpacking mixed plugins, with a list of what is lost
- Pros: the user can take a skill out of a plugin they half want, and see its parts on the Harnmap.
- Cons: the library holds a plugin's parts that behave differently from the plugin; scaffold silently differs from a plugin install.
- Rejected by the user (2026-11-11): parity with a plugin install comes first.

### Ask Claude whether a plugin can be split
- Pros: Claude reads a plugin the way a person would and can judge cases no rule lists.
- Cons: the repo is untrusted and can steer the answer; sending content is opt-in (`CONTEXT.md`); updates must read a source the same way every time (ADR-001).
- Rejected: the allow-list gives a safe answer without Claude.

## Consequences
- Fewer plugins unpack. Skill packs (`addyosmani/agent-skills`, `mattpocock/skills`) still do; impeccable, and any plugin with hooks or plugin variables, stays whole and shows as one box on the Harnmap.
- The import's Choose step and the library's Unpack action read the reason from the one `plugin` module (ADR-004); `plugin.part-needs-plugin` on a single part becomes a reason for the whole plugin.
- Plugins already unpacked in a library that would now stay whole keep their parts until the user acts: the library marks them, and offers to replace them with the whole plugin.
- Showing a whole plugin's members on the Harnmap, without unpacking it, is a separate decision.
