# ADR-007: An artifact carries the files it links to outside its own folder

## Status
**Superseded by [ADR-008](ADR-008-plugin-parts-as-trimmed-plugin.md) (2026-11-17) for plugin parts only; it still holds for artifacts that come from no plugin.** Accepted (user decision 2026-11-17).

## Date
2026-11-17

## Context
Harneo copies an artifact's own folder (a skill's `skills/<name>/`, an agent's file, a command's file) into the library and installs exactly that. Skill packs often keep material several skills share at the repo root and link to it with a relative path. `addyosmani/agent-skills` does this: `skills/code-review-and-quality/SKILL.md` says "see `../../references/security-checklist.md`", and 13 files across its skills point into `references/`.

A review of a project scaffolded from such skills (2026-11-17, `~/projects/test`) found every one of those links broken: `.claude/skills/<name>/../../references/` is `.claude/references/`, which Harneo never wrote. Agents spend tool calls on the missing files or skip the checklists. No import check warns: ADR-006 rule 4 looks for plugin variables, not plain relative links, and a repo imported as loose skills is never judged by ADR-006 at all.

A whole plugin does not have the problem, since its install brings its own folder. The gap is every artifact Harneo copies on its own: loose skills, agents and commands, and the parts of an unpacked plugin.

## Decision
- **Find the links at scan time.** For a skill, agent, command or output style, read its text files for relative paths that start with `../` (in Markdown links, in backticks, or bare). A path counts when, resolved from the file that names it, it reaches a regular file that exists in the scanned source, lies outside the artifact's own folder, and stays inside the scan root. The artifact records those files, root-relative and sorted, as its **linked files**. Links are followed one level: a carried file's own links are not.
- **Copy them with the artifact.** The library copy stores each linked file under `_linked/<root-relative path>`, through the same safety checks as the artifact's own files (no symlinks, no absolute paths, size limit, secret scrubbing), and the audit reads them like any other file of the artifact.
- **Install them where the link points.** A linked file goes to the project path the link resolves to from the installed artifact: the file's position relative to the artifact's source folder, applied to its install folder. `skills/x/` → `.claude/skills/x/` puts `references/a.md` at `.claude/references/a.md`. The target must pass `in_harness` (under `.claude/`, `scripts/`, or a file Harneo knows by name); a link that would land anywhere else is not carried, and the import warns with the path.
- **Shared, not conflicting.** Two artifacts that carry the same project path with the same bytes write it once; HARNESS.md and drift list it under the first artifact in profile order. Different bytes at one path are a layout conflict, as two artifacts claiming one path are today.
- **Updates see them.** A change upstream to a linked file makes the artifact outdated, the same as a change to one of its own files.

## Alternatives Considered

### Warn only, copy nothing
- Pros: no new storage or install rules.
- Cons: the scaffolded harness stays broken; the user can only fix it by hand, outside Harneo, and drift then reports the hand-made files as unknown.
- Rejected by the user (2026-11-17): Harneo should produce a working harness.

### Treat the source's shared folder as its own artifact
- Pros: one library entry for `references/`, picked once.
- Cons: the user has to know which skills need it; a profile without it is still broken; the Library gets entries that are not Claude Code artifacts.
- Rejected: the link already says which skill needs which file.

### Rewrite the links to point inside the skill and copy the files there
- Pros: everything stays in the skill's folder.
- Cons: changes the artifact's bytes, so the library copy no longer matches its source; two skills sharing a checklist get two copies.
- Rejected: Harneo installs what the source says, byte for byte, except where a decision already says otherwise (manual only).

## Consequences
- An artifact gains a list of linked files; the library copy gains a `_linked/` folder. Artifacts imported before this have no linked files until they are updated or re-imported.
- Projects scaffolded from such skills get folders like `.claude/references/` that hold no artifact of their own; HARNESS.md lists them under the skill that brought them.
- A link to a file outside the harness (for example `../../../docs/guide.md` from a skill, which would land at `docs/guide.md`) stays broken, and the import says so.
