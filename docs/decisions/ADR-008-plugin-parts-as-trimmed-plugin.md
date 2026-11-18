# ADR-008: Pick a plugin's parts freely; Claude finds what they need; install them as a trimmed plugin

## Status
Accepted (user decision 2026-11-17). Supersedes ADR-004 (unpack by default), ADR-005 (whole plugin folder) and ADR-006 (unpack allow-list). Changes ADR-007 for plugin parts: a part's supporting files now travel inside its trimmed plugin, not as `_linked/` copies; ADR-007 still holds for artifacts that come from no plugin. Extends ADR-001's tier 3.

## Date
2026-11-17

## Context
Four ADRs in three weeks tried to answer one question with static rules: which of a plugin's files does each part need, and when can a part leave its plugin?

- ADR-004 unpacked plugins by default with a deny-list of reasons to keep one whole.
- ADR-005 kept a whole plugin's folder because scripts reach data files through paths built at run time (`mattpocock/skills`).
- ADR-006 switched to an allow-list after `pbakaus/impeccable`. On 2026-11-17 it kept `addyosmani/agent-skills` whole because its manifest gained `experimental` and its commands call `agent-skills:incremental-implementation`.
- ADR-007 carried files that parts link to with `../`.

Each rule fixed the last repo and broke the next. mattpocock needed folders a rule would drop; agent-skills has `docs/` that should be dropped and `references/` that must be kept. No static rule tells prose for people from material a skill reads.

The user's goal is fixed: import a plugin, drop the parts they don't want, see and place each kept part on its own (Library, profiles, Harnmap), and get a scaffolded project where the kept parts work exactly as in the plugin.

Two parts copied as loose files into `.claude/skills/` and `.claude/commands/` cannot work exactly as in the plugin: they lose the `<plugin>:` prefix that commands use to call skills, `${CLAUDE_PLUGIN_ROOT}`, and the plugin's folder structure. A test on 2026-11-17 (Claude Code 2.1.326, this repo) showed a third way. A plugin folder placed at `.claude/skills/<plugin>/` with its `.claude-plugin/plugin.json` is loaded in place as a plugin (origin `@skills-dir`):

- its skills and commands are namespaced (`demo-pack:alpha`, `demo-pack:go`);
- `../../references/ref.md` resolved inside it;
- nothing was registered machine-wide (`claude plugin list` did not list it).

The folder must be trusted; in an untrusted folder Claude Code skipped it silently. The docs say such a plugin does not load background monitors and skips MCP bundles.

## Decision
- **Any plugin can be split.** Import always offers its parts, and the user keeps or drops each one. The allow-list and the "must stay whole" verdicts of ADR-006 are removed. The only plugins kept whole are those with features a `.claude/skills/` plugin does not support: `monitors`, MCP servers (`mcpServers` or `.mcp.json`), LSP servers (`lspServers` or `.lsp.json`) and `userConfig` (checked 2026-11-18, see Consequences). The screen says why, and they install from their marketplace as today.
- **Claude finds the parts and what they need, on by default.** For every plugin and every repo it imports, Harneo asks Claude (ADR-001 tier 3, setting `import.layoutWithClaude`, on by default; the user's choice of 2026-11-17) to list:
  - every artifact, and
  - for each artifact, the files and folders outside its own folder that it needs to work: data its scripts read, references it links to, templates.

  Claude also marks what is not needed (docs, tests, evals, CI, other agents' configs). The answer is a proposal in a fixed schema, validated like tier 3 today: paths exist, stay inside the root, cross no symlink. Manifest components (tier 1) are always parts, whatever Claude says. A part Claude names that the manifest lacks is shown as a proposal. The answer is stored with the source and replayed by updates. Claude is asked again only when the source's file tree changes outside the stored answer, and the import shows what changed.
- **Without Claude:** when the setting is off or the call fails and the user chooses to continue, a part needs everything in the plugin's folder that no other part claims. This is safe (never missing a file) but broad, and the import says so.
- **Parts are library items.** Each kept part is its own item in the Library, profiles and the Harnmap. It records its plugin (name, manifest, source) and its supporting paths. Supporting files are stored once per plugin version, not once per part.
- **Scaffold writes a trimmed plugin.** The parts of one plugin that a profile holds are written to `.claude/skills/<plugin>/`:
  - the plugin's manifest, with component paths narrowed to the kept parts and keys that point only at dropped parts removed;
  - the kept parts, at their paths in the plugin;
  - the union of their supporting files.

  Everything is byte for byte from the source, except the manifest and the manual-only flag. Nothing goes in `enabledPlugins`, no marketplace is registered, and `claude plugin install` is not run. Dropping the plugin's last part from a profile deletes the folder on refit. HARNESS.md and drift treat the folder as that plugin's parts.
- **Dependencies between parts are shown, not decided.** When a kept part names a dropped part (`<plugin>:<name>`, `/<plugin>:<name>`), import and the profile builder show "`/build` needs `incremental-implementation`" and let the user keep it or drop both.
- **Trust is stated.** The Done step of Scaffold and Refit says the project must be trusted in Claude Code for its plugins under `.claude/skills/` to load.

## Alternatives Considered

### Keep ADR-006's allow-list, add the next exception
- Rejected by the user (2026-11-17): the fourth iteration of static guessing; it keeps agent-skills whole, so the user cannot drop parts.

### Copy parts loose into `.claude/skills/` and `.claude/commands/` (ADR-004 before ADR-006)
- Pros: no plugin folder in the project.
- Cons: namespaced calls, `${CLAUDE_PLUGIN_ROOT}` and relative structure break, as `~/projects/test` showed.
- Rejected: the kept parts must work as in the plugin.

### A project-local marketplace (`extraKnownMarketplaces` with a `directory` source) and `claude plugin install --scope project`
- Pros: an install path Claude Code documents for plugins.
- Cons: the marketplace registry is per user and allows one marketplace per name, and the cache is keyed by marketplace, plugin and version (`unknown` for a local folder). Two projects with different trims of one plugin would overwrite each other. Every scaffold would also need a machine-wide install step.
- Rejected: a scaffold must not change other projects.

### Claude only for unplaced files (ADR-001 as written)
- Rejected: the files that decide whether a part works (agent-skills' `references/`, mattpocock's data folders) sit inside a plugin's scope, where Claude was never asked.

## Consequences
- The `plugin::must_stay_whole` allow-list, the "must stay whole" marks and *Replace with the whole plugin* (#274, #292) go. The warning before Scaffold and Refit is kept only for plugins kept whole by the rule above. #292's uninstall of dropped plugins still applies to plugins installed from a marketplace.
- Import costs one Claude call per new source by default. A source whose stored answer still matches costs nothing on update.
- Libraries holding parts imported before this record no supporting paths and no manifest. They are marked *Re-import to update*, and until then install as loose copies, as before.
- A user who has not trusted a scaffolded project sees none of its trimmed plugins, without an error from Claude Code. The Done step is the only place Harneo can say so.
- Checked on 2026-11-18 with a real `claude -p --model haiku` (Claude Code 2.1.327) in throwaway projects inside a trusted folder, one `.claude/skills/<plugin>/` plugin per case, reading `--debug-file`:
  - **Trimmed plugin loads:** a plugin of 3 skills and 1 command (`skills` and `commands` narrowed in the manifest, `references/` beside them) loaded as `agent-skills:build` and the 3 skills; the dropped skills and command were not offered. `${CLAUDE_PLUGIN_ROOT}`-style paths and `../../references/` links resolve inside it.
  - **Hooks: honoured.** A `hooks` path in the manifest ran its `UserPromptSubmit` command, with `${CLAUDE_PLUGIN_ROOT}` resolved. Hooks are not parts, so a scaffold carries a hooks file only when Claude lists it as needed; the manifest keeps its `hooks` key only when that file is in the folder.
  - **`userConfig`: not honoured.** The plugin loads, but Claude Code never asks for the values (a `required: true` key blocked nothing) and `${user_config.token}` stays as written in the skill. A plugin that declares `userConfig` is therefore kept whole, like one with monitors, MCP or LSP servers.
  - **`dependencies`: enforced, not satisfied by Harneo.** A dependency that is not installed makes the whole plugin unavailable (`dependency-unsatisfied`, nothing loads). Claude Code does honour the key, so the plugin is not kept whole for it, but Harneo installs nothing for a `.claude/skills/` plugin, so such a trimmed plugin loads only on machines where the dependency is already installed. Whether to keep these plugins whole too is left to the user (an open question of #296).

## Amendment 2026-11-18 (#301): a plugin starts whole; Claude only when the user splits it
User decision after testing `pbakaus/impeccable` and `addyosmani/agent-skills`: asking Claude about every plugin on arrival cost a call (and a wait) the user rarely needs, and its proposals duplicated a plugin's own contents.
- **Every plugin starts as Whole plugin** (`ScanOptions::unpack_by_default` is now `false`). Splittable ones offer **Pick parts**.
- **Claude is asked on its own only when the user picks parts** of a plugin with no stored answer (or a tree changed outside it). A plugin taken whole never asks.
- **Files Harneo cannot place are sent to Claude on their own only in a source with no plugin** (loose prompts). Next to a plugin they are mostly the author's working copies and tooling; the "couldn't place" note keeps its button for when the user wants them.
- **A proposal with the type and name of a plugin part in the same source is a duplicate**: set aside, with the reason, and skipped on updates. **Everything else Claude proposes starts excluded**; the user restores what they want.
- While Claude works, the import shows a blocking dialog with a spinner and **Cancel import** (the calls have no timeout).
