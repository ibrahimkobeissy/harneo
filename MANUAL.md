# Harneo manual

What each screen does, screen by screen. The [README](README.md#how-it-works) has the six-step overview; [Privacy](README.md#privacy) lists everything Harneo reads outside its library and everything that leaves your machine.

- [Getting around](#getting-around)
- [Import](#import)
- [Library](#library)
- [Profiles](#profiles)
- [Harnmap and Harnfit](#harnmap-and-harnfit)
- [Scaffold a project](#scaffold-a-project)
- [Refit a project](#refit-a-project)
- [Projects](#projects)
- [Security](#security)
- [Cleanup](#cleanup)
- [Settings](#settings)

## Getting around

On first start, with an empty library, Harneo shows a welcome card (Library → Profile → Project) and an **Import your first artifacts** button.

| Keys | Does |
| ---- | ---- |
| Ctrl K (⌘K on macOS) | Search and commands: jump to any screen or artifact |
| Ctrl B (⌘B on macOS) | Show or hide the sidebar |
| ? | The list of shortcuts |
| / | Search the Library |
| ← ↑ → ↓ | Move between cards in the Library |
| Home, End | First or last card |
| Esc | Close a dialog or the palette |

The sidebar's **Projects** badge counts the projects that need attention (drift), not every project.

## Import

Importing copies artifacts into Harneo's library. Nothing in your projects or in `~/.claude` changes.

1. **Source.** Your `~/.claude`, a git URL (Harneo records the URL, branch, subfolder and commit), or a folder (any repo or folder with a `.claude/` directory, `CLAUDE.md`, `.mcp.json` or loose `SKILL.md` files).
2. **Choose artifacts.** Everything found is listed by type with its trust score; take out what you don't want. Risky and dangerous ones start excluded.
   - **Plugins** are taken **whole**, or with **Pick parts** you choose their skills, agents and commands one by one. A plugin already in the library whole can't also be imported in parts.
   - **Claude-managed folders are skipped**: skills Claude syncs from your account (`skills/synced/`) and mods in development (`dev-mods/`). **Include them** brings them in for this import. Skills Claude Code deleted (`.trash/` folders) are never offered.
   - **Large files** (over 1 MiB) are listed but not copied. Tick **Copy large files too** to copy them, up to 100 MiB each; large files are not security audited.
   - **Files Harneo couldn't place** are listed; **Ask Claude what's in this repo** proposes where they belong (see [Settings](#settings)).
   - When **Confirm security findings with Claude** is on, each finding gets Claude's verdict (for example *Claude: not a threat*) before you can continue.
3. **Review.** The audit summary, what each artifact can reach (files, network, shell, environment variables), proposed bundles and **suggested tags**. Tags are suggested from each artifact's own keywords, locally; remove any or add your own.
   - **Harnfit** (optional): pick a profile with a drawn map, then **Harnfit** any candidate to see whether it fits before you import it ([Harnmap and Harnfit](#harnmap-and-harnfit)).

## Library

Everything you imported, as cards or a list, with filters by type, trust, status (current, unused, outdated), invocation, tag and source.

- **Library health**: how many artifacts are current, unused, outdated or have findings.
- **Sessions using library artifacts**: usage over the last 8 weeks, counted from your Claude Code session logs (read only; see [Privacy](README.md#privacy)).
- **Check for updates** compares every artifact with its source. Outdated ones are flagged; nothing is updated without you.
- **Save as smart collection** keeps the current filters as a collection in the sidebar. **Tag…** and **Delete…** act on every artifact the filters show.
- **Find bundles** asks Claude which artifacts only work together (see [Settings](#settings)).

Opening an artifact shows its description, tags, trust score, findings and reach, its bundle, usage, the profiles that use it, where it came from, and its files. A file opens in your default app; a script is shown in your file manager instead, so that opening it never runs it.

- **Manual only** stops Claude from invoking the artifact on its own: it sets `disable-model-invocation: true`, so it runs only when you call it.
- **Add to…** puts it in a profile or a collection.
- **Harnfit in…** fits it into a profile's map ([Harnmap and Harnfit](#harnmap-and-harnfit)). Each profile row shows its saved verdict or "not fitted yet"; the card carries a verdict badge per profile with a saved fit.

## Profiles

A profile is a named selection of library artifacts, installed into projects.

- Adding an artifact adds its whole bundle (the subagents and scripts it needs).
- Pick at most one **Settings template**: the project's whole `.claude/settings.json` (permissions, hooks and their scripts, status line, style, plugins). **None** leaves the project's settings alone.
- Add **MCP servers**. Secrets are kept as `${VAR}` placeholders only.
- **The project it produces** lists every file the profile writes.
- Profiles are versioned (v1, v2, …), can be cloned, exported as one file, and imported back with **Import profile file**.
- Each profile has a **category** (Software development, Research, Writing & content, Video production, Social media, Data & analytics, Design, or Custom, described in a few lines), which gives its Harnmap a template. The builder's **Harnmap card** says whether the map is up to date and how many artifacts are placed.
- Export can include the map, its history and the saved fits, so the profile arrives with its Harnmap ready and no tokens spent.

## Harnmap and Harnfit

**Harnmap** draws a profile as the workflow it serves; **Harnfit** checks a new artifact against that drawing before you add it. Both run through your own `claude` CLI on your plan, only when you press the button, and say what they send next to it ([Privacy](README.md#privacy)). The model is Opus unless you pick Sonnet in [Settings](#settings).

### Draw the map

Open a profile's **Harnmap**. Before the first drawing it shows the category's template; type an optional description of how you work ("spec first, TDD build, review with two agents…") and press **Draw** (`Ctrl+Enter`). Claude usually takes one to three minutes; then the placements replay onto the map and the drawing is saved.

Every category has its own shape: a line with loops (software, writing, data), a cycle (research, social media), parallel tracks (video) or a double diamond (design). Claude may rename, add or remove stages, and says so.

### Read the map

- **The line**: the stages, each with the artifacts Claude placed there. Dashed loops (for example *tests fail* back to Build) are Claude's reading of your workflow, not rules.
- **Toolbelt**: artifacts you reach for at any stage. **Floor**: what is always on (rules, `CLAUDE.md`, settings, output styles). The settings template's hooks run as a rail along the whole line.
- **Not everything is a step.** Artifacts Claude left off the map wait under **Unplaced** in the side panel.
- **Select a node** to see *Why Claude put it here*, the facts read from its files, its note and its **Natural next steps**: the likely next artifacts, numbered most likely first, each with Claude's one-line reason and ringed on the map.
- A skill that only runs through another (a child) sits indented under its parent; **Detach** separates it. A skill only Claude invokes carries a *model only* mark.
- Zoom (40–150 %, `Ctrl` + wheel), drag to pan, **Fit**; switch the Line, Toolbelt and Floor layers and filter by **Types**. **List** shows the same placements as a table. `]` hides the side panel.

### Change it

Drag a node to another stage, or use **Move to**; drag an unplaced artifact onto the map and say why. Edit notes, **Hide** or **Remove from map**; **Stages** renames, reorders, adds and removes stages. Anything you place or move is **pinned** and stays put on a re-draw. Edits save as you make them and never change the profile itself.

When the profile changes, the map says it is **out of date** and lists what changed; **Re-draw** or place the new artifacts by hand. A re-draw never overwrites: **Map history** keeps every drawing, with **Preview** and **Restore**.

### Harnfit: try it before you add it

Start from a library artifact (**Harnfit in…**) or from the import review. The profile's map fades and the candidate appears as a dashed **NEW** node where Claude would put it, with a verdict:

- **Fits**: where it goes and why.
- **Fits with changes**: the change it needs (for example, a missing file it depends on, or a description too thin to route).
- **Conflicts**: the artifacts it clashes with, drawn in red on the map, and a proposed change if you still want it.

Each verdict quotes the lines it relies on (`file:line`, checked word for word against the files). **Add to profile** adds it where Claude placed it; **Place elsewhere** lets you drag the NEW node first; **Discard** leaves the profile alone. Fits are saved per profile and candidate, so reopening one costs nothing; when the profile changes, a fit is marked out of date (**Run again** or keep the old verdict). Harnfit needs a drawn map, takes one candidate at a time, and doesn't fit a plugin kept whole yet (pick its parts instead).

## Scaffold a project

Writes a profile into a folder that has no harness yet.

1. **Folder** (new or empty).
2. **Profile.**
3. **Preview**: a dry run of every file, readable before anything is written.
4. **Done**: the files are written with a `HARNESS.md` listing what was installed and why, and the project is registered.

Plugins are not copied: `.claude/settings.json` enables them and Harneo installs them for the project through your `claude` CLI, from each plugin's own marketplace.

## Refit a project

Fits a profile into a project that already has a harness.

1. **Folder** and **Profile.**
2. **Compare**: each file in the project and in the profile, with **Add / Skip** for new files, **Keep / Delete** for files only the project has, and **Replace / Keep** where both differ. A replaced file that loses the project's own `CLAUDE.md` or `settings.json` content is flagged in red. `CLAUDE.local.md` and `settings.local.json` are never touched.
3. **Preview**: the content diff of every change.
4. **Done**: every replaced or deleted file is backed up first. **Restore originals** puts the backed-up files back at once; files the refit added stay, and the project may then show drift.

## Projects

Every registered project with its profile, status (in sync, or drift) and last apply.

A project's page has three tabs:

- **Drift**: files that changed since the profile was applied. Re-apply the profile's version, or promote the project's change to the library.
- **Installed files**: what the profile wrote, readable in place.
- **Backups**: one entry per refit or re-apply, with **Restore** and **Delete**. Backups live in the data folder's `backups/`.

The page's actions: **Open** (the folder in your file manager), **Import from this project**, **Switch profile**, and under **More actions**, **Remove the harness**, which restores the originals. When the profile has a newer version, the page offers the upgrade: only the difference is applied, with a preview first.

## Security

The audit of the whole library: trust scores (Trusted 80–100, Needs review 60–79, Risky 40–59, Dangerous 0–39), findings by severity, and what artifacts can reach: network hosts, shell commands, environment variables and files outside their own folder. Selecting a reach card filters the table. **Findings by artifact** lists each finding with its file and line. **Confirm now** sends the findings Claude hasn't judged yet to Claude; findings Claude dismissed are listed with its reason in the artifact's detail panel and left out of the counts.

## Cleanup

Keeps the harness lean: risky artifacts, unused ones (no use for the number of weeks set in Settings), repeated ones and **duplicate names** (the same artifact imported from two sources), each with a link to it in the Library.

## Settings

- **Data**: the data folder (library, profiles, project registry, backups); the Claude Code **session logs** Harneo reads for usage (read only, never written; **Re-index** reads them again); **Unused after N weeks** (1 to 52, default 4).
- **Claude**: whether your `claude` CLI was found, the **model** that reads your artifacts (Haiku, the default; Sonnet; Opus), and the **Harnmap and Harnfit model** (Opus, the default; Sonnet). Each feature that sends text to Claude has its own switch and says exactly what it sends: confirming security findings (on), detecting bundles and tags at import (off), asking about repo layouts at import (on).
- **Background**: **Check for updates at start** (on: at most once a day, using git only; nothing is updated without you) and **Pause live rescan** (Harneo watches the library and each project's harness files and notices changes; it only reads).
- **Diagnostics**: error reports to a Sentry project of your own. Off unless you set `HARNEO_SENTRY_DSN` ([BUILDING.md](BUILDING.md#error-reporting-optional)); turned off here, it stays off.
- **Danger zone**: reset Harneo.
