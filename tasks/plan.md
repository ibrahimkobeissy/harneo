# Implementation Plan: app-shell

> Module `app-shell` from [`spec/capabilities.md`](../spec/capabilities.md). Spec: [`spec/app-shell.md`](../spec/app-shell.md). Quality bar: [`CONSTRAINTS.md`](../CONSTRAINTS.md).
> **Tasks are tracked in GitHub Issues** (`ibrahimkobeissy/harneo`, Project 5) and built by the `ticket-flow` cycle. This file holds the approach and the ordered index of issues, never a duplicate checklist.
> Status: **approved 2026-10-21**; tasks created as issues #11–#23.

## Overview

Build Harneo's base: a Cargo workspace (`harneo-core`, `harneo-cli`, `harneo-app`) and a React frontend that run on Linux from source; one shared core proven by the CLI and the About panel showing the same version and data dir; the app frame and navigation; the quality gates from `CONSTRAINTS.md` (`check:fast` / `check:task` / `check:full`) with CI; the design system created with Impeccable; and the build guide.

## Architecture Decisions

- **Workspace layout, crate names, app id `io.harneo`, stack and conventions:** as `spec/app-shell.md` sets them. Nothing new is decided here.
- **Vertical slices first.** The first tasks push one value (the version and data dir) through every layer (core → CLI, core → IPC → React) so the whole stack is proven before anything is added to it.
- **Gates arrive early, one script at a time.** `check:fast` lands right after the scaffold, then `check:task`, then `check:full` + CI. Until a gate exists, the task's own verification commands stand in for it, and the Floor applies by review.
- **Tasks that edit `package.json` scripts are chained** (T5 → T6 → T7 → T8 → T9), so parallel subagents never conflict on it.
- **The design is done.** `DESIGN.md` and the validated clickable prototype (`design/prototype/`) are the reference for every UI ticket. T11 turns `DESIGN.md` into tokens and is an ordinary ticket; UI tickets run under Impeccable's checks and get a `/impeccable critique` in the running app.
- **The frame follows the prototype:** Security, Collections, Cleanup, the shortcuts button and Search are in the sidebar from the start (placeholders until their modules land); About lives on the Settings screen.

## Dependency graph

```
T1 scaffold ── T11 tokens from DESIGN.md (fonts, icons bundled)
 ├── T2 core version+paths → CLI ── T3 app_info IPC → About panel ── T4 app frame
 │                                                                     └── T12 components + styled frame (+T11)
 └── T5 lint/format + check:fast ── T6 floor guard ── T7 check:task (+T2,T3) ── T8 architecture gates (+T4)
                                                                                 └── T9 check:full + CI (+T6) ── T10 coverage ratchet
                                                                                                                └── T13 docs (+T12)
```

Parallel after T1: T2 and T5. T11 can start right after T1, in parallel with T2 and T5.

## Task list

Tasks are GitHub issues on Project 5; their bodies hold the acceptance criteria and verification. Build order follows the dependencies; the `ticket-flow` cycle picks Ready issues.

### Phase 1: Foundation (one value through every layer)

- [#11](https://github.com/ibrahimkobeissy/harneo/issues/11) T1: Scaffold the Tauri 2 + React workspace
- [#12](https://github.com/ibrahimkobeissy/harneo/issues/12) T2: Core `version()` and `paths::data_dir()`, shown by the CLI
- [#13](https://github.com/ibrahimkobeissy/harneo/issues/13) T3: `app_info` IPC end to end, shown in the About panel
- [#14](https://github.com/ibrahimkobeissy/harneo/issues/14) T4: App frame and navigation

**Checkpoint: Foundation** (after T1–T4)
- [ ] The window opens; every sidebar entry opens its screen; the About panel and the CLI show the same version and data dir (spec criteria 1, 2, 3, 5)
- [ ] `cargo test --workspace` and `npx vitest run` pass
- [ ] The user runs the app before Phase 2 continues past T5–T6

### Phase 2: Quality gates (`CONSTRAINTS.md` made real)

- [#15](https://github.com/ibrahimkobeissy/harneo/issues/15) T5: ESLint, Prettier and `check:fast`
- [#16](https://github.com/ibrahimkobeissy/harneo/issues/16) T6: Floor guard `scripts/check-floor.sh`
- [#17](https://github.com/ibrahimkobeissy/harneo/issues/17) T7: `check:task` with changed-line coverage
- [#18](https://github.com/ibrahimkobeissy/harneo/issues/18) T8: Architecture gates
- [#19](https://github.com/ibrahimkobeissy/harneo/issues/19) T9: `check:full` and CI
- [#20](https://github.com/ibrahimkobeissy/harneo/issues/20) T10: Record the coverage baseline (`/constraints ratchet`)

**Checkpoint: Quality gates** (after T5–T10)
- [ ] `check:fast`, `check:task` and `check:full` run every check `CONSTRAINTS.md` lists; CI is green on `main` (spec criteria 6, 10)
- [ ] Every gate was seen failing on a planted violation
- [ ] From here, every ticket passes `check:task` before review and `check:full` before merge

### Phase 3: Design system in code, and docs

- [#21](https://github.com/ibrahimkobeissy/harneo/issues/21) T11: Design tokens from `DESIGN.md`
- [#22](https://github.com/ibrahimkobeissy/harneo/issues/22) T12: Base components and the frame styled like the prototype
- [#25](https://github.com/ibrahimkobeissy/harneo/issues/25) T14: Setup script for building Harneo from source (and contributor tools)
- [#23](https://github.com/ibrahimkobeissy/harneo/issues/23) T13: `BUILDING.md`, README and CLAUDE.md commands

**Checkpoint: app-shell complete** (after T11–T13)
- [ ] Every success criterion in `spec/app-shell.md` (1–10) is met, and the frame matches the prototype
- [ ] `check:full` and CI green on `main`
- [ ] The user reviews the running app; then `spec/capabilities.md` marks app-shell done and the next module's spec starts

## Spec coverage

| Spec success criterion | Tasks |
|---|---|
| 1. Window opens following `BUILDING.md` | T1, T13 |
| 2. Sidebar and screens from the prototype's frame | T4, T12 |
| 3. About panel shows `app_info` | T3 |
| 4. OS theme, all values from tokens | T11, T12 |
| 5. CLI and About show the same values | T2, T3 |
| 6. `check:*` scripts and CI | T5–T9 |
| 7. `--no-bundle` binary, no installer | T1 |
| 8. `BUILDING.md` per OS, starting with the setup script | T14, T13 |
| 9. Commands documented, Impeccable enabled | T13 (Impeccable already done) |
| 10. Coverage baseline recorded | T10 |

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Tauri 2 / WebKitGTK system packages missing or mismatched on Linux | High | T1 goes first; whatever it needed is written into `BUILDING.md` in T13 |
| Code written from Tauri v1 memory (APIs changed a lot in v2) | Med | Check Tauri 2's official docs while implementing T1 and T3 |
| Rust compile times push `check:task` over 3 minutes | Med | `cargo test -p` for touched crates only; measured in T7; if over, raise it with the user rather than drop a check |
| `diff-cover` merging Rust and TS lcov paths | Med | T7 must show a failure on both sides before it counts as done |
| Gate tools (gitleaks, osv-scanner, semgrep, cargo-llvm-cov, diff-cover) not installed on the dev machine | Low | Listed as contributor prerequisites in T13; the scripts fail with an install hint when a tool is missing |
| Parallel subagents conflicting on `package.json` | Low | The tasks that edit it are chained by dependencies |

## Open Questions

None.

---

# Implementation Plan: artifact-model

> Module `artifact-model` from [`spec/capabilities.md`](../spec/capabilities.md). Spec: [`spec/artifact-model.md`](../spec/artifact-model.md). Quality bar: [`CONSTRAINTS.md`](../CONSTRAINTS.md).
> **Tasks are tracked in GitHub Issues** (`ibrahimkobeissy/harneo`, Project 5) and built by the `ticket-flow` cycle. Task ids are `A1`–`A12` so they don't clash with app-shell's `T1`–`T14`.
> Status: **approved 2026-10-21**; tasks created as issues #28–#39.

## Overview

Teach `harneo-core` to find, parse and check the 10 artifact types in a local folder, behind an agent-neutral `AgentAdapter`, safely (the folder is untrusted), and expose it as `harneo scan <path> [--json]`. No screens.

## Architecture Decisions

- **One thin path first.** A1 builds the types, the adapter trait, frontmatter parsing and one type (skills) end to end; A2 puts `harneo scan` on top. Every later type is then "one more file in `claude/kinds/` plus one dispatch line", which keeps parallel tickets small and their merges trivial.
- **Safety is early, not last.** A3 (the safe walk and the hostile fixture) comes straight after the first slice, so every later parser reads files only through `walk.rs` and inherits the limits.
- **The types are complete from A1.** `ArtifactKind` lists all 10 types and `Artifact`/`Reference`/`Diagnostic`/`ScanReport` are final in A1, so later tickets add parsers, not fields. A field that turns out to be missing is a change to A1's types, made by the orchestrator, not inside a parallel ticket.
- **One secret detector.** A6 (Settings) writes the literal-secret check in a shared helper; A7 (MCP) reuses it, so A7 is blocked by A6.
- **Cross-artifact work waits for the parsers:** reference resolution and duplicate checks (A10) need the references the parsers produce; the all-shapes fixture test and `install_path` for every type (A12) close the module.
- **Dependencies** are the ones the spec approves (`serde`, `serde_json`, `serde-saphyr`, `sha2`, `ts-rs`, `tempfile` dev). Versions checked on crates.io when A1 starts.

## Dependency graph

```
A1 types + adapter + frontmatter + skills ─┬─ A2 harneo scan CLI
                                            ├─ A3 safe walk + hostile fixture ─┬─ A11 digest
                                            │                                  └─ A9 plugins, marketplace, ~/.claude, loose skills
                                            ├─ A4 rules, commands, subagents, output styles
                                            ├─ A5 CLAUDE.md + @imports (+A3)
                                            ├─ A6 Settings + secret helper ── A7 MCP servers
                                            └─ A8 workflows
A4, A5, A6, A7, A9 ── A10 reference resolution + duplicate checks
A2, A8, A10, A11 ── A12 install_path + all-shapes fixture test
```

Parallel after A1: A2, A3, A4, A6, A8. After A3: A5, A9, A11.

## Task list

Tasks are GitHub issues on Project 5; their bodies hold the acceptance criteria, verification and `Blocked by` links. The `ticket-flow` cycle picks Ready issues.

- [#28](https://github.com/ibrahimkobeissy/harneo/issues/28) A1: Artifact types, `AgentAdapter`, frontmatter, and skills found in a folder
- [#29](https://github.com/ibrahimkobeissy/harneo/issues/29) A2: `harneo scan <path> [--json]`
- [#30](https://github.com/ibrahimkobeissy/harneo/issues/30) A3: Safe walk and the hostile fixture
- [#31](https://github.com/ibrahimkobeissy/harneo/issues/31) A4: Rules, commands, subagents, output styles
- [#32](https://github.com/ibrahimkobeissy/harneo/issues/32) A5: CLAUDE.md and its `@imports`
- [#33](https://github.com/ibrahimkobeissy/harneo/issues/33) A6: Settings template and the literal-secret check
- [#34](https://github.com/ibrahimkobeissy/harneo/issues/34) A7: MCP servers from `.mcp.json`
- [#35](https://github.com/ibrahimkobeissy/harneo/issues/35) A8: Workflows
- [#36](https://github.com/ibrahimkobeissy/harneo/issues/36) A9: Plugins, marketplaces, `~/.claude`-like folders, loose skills
- [#37](https://github.com/ibrahimkobeissy/harneo/issues/37) A10: Reference resolution and cross-artifact checks
- [#38](https://github.com/ibrahimkobeissy/harneo/issues/38) A11: Artifact digest
- [#39](https://github.com/ibrahimkobeissy/harneo/issues/39) A12: `install_path` for every type and the all-shapes fixture test

## Checkpoints

**Checkpoint: first slice** (after A1–A3)
- [ ] `harneo scan` on a folder of skills prints them; the hostile fixture is safe
- [ ] `check:full` passes; coverage ratchet holds

**Checkpoint: every type** (after A4–A9)
- [ ] All 10 types parse with their checks; the secret warning never leaks the value (spec criteria 2, 3, 4)
- [ ] The user runs `harneo scan` on a real source of their choice and says what looks wrong

**Checkpoint: artifact-model complete** (after A10–A12)
- [ ] Every success criterion in `spec/artifact-model.md` (1–9) is met
- [ ] The user runs `harneo scan` on their `~/.claude`, an Addy `agent-skills` clone and the Impeccable plugin (criterion 8); anything wrong becomes a fixture and a fix
- [ ] `spec/capabilities.md` marks artifact-model done; the `library` spec starts

## Spec coverage

| Spec success criterion | Tasks |
|---|---|
| 1. Every shape lists the expected artifacts | A9, A12 |
| 2. Every check has a code and both tests | A1, A4–A8, A10 |
| 3. Hostile fixture safe | A3 |
| 4. Literal secret never leaks | A6, A7 |
| 5. `install_path` per type | A12 |
| 6. Deterministic report and digests | A11, A12 |
| 7. TS bindings compile | A1 |
| 8. Real sources, under 2 s | checkpoint (user) |
| 9. `check:full` and the ratchet | every task |

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Claude Code's file formats differ from what we assume (field names, which are required) | High | Each kind ticket checks the current docs at code.claude.com before coding, and HarnessKit's Claude adapter for reference (Apache-2.0, attribute anything borrowed) |
| `serde-saphyr`'s API or limit options aren't what the spec expects | Med | A1 verifies it first; if it falls short, stop and raise it with the user before picking another crate |
| Parallel kind tickets conflicting in `claude/mod.rs` dispatch | Low | A1 sets the one-line-per-type dispatch; the orchestrator resolves the one-line rebases |
| Hostile fixture tripping gitleaks or the secret-path guard hook | Med | Secret-pattern files are generated in a temp dir at test time, never committed |
| Rust coverage ratchet (74.58%) falls | Low | Parsers are pure and unit-tested; every diagnostic code has both tests |

## Open Questions

None for this module. The literal-secret import behaviour is the `library` spec's question (see `spec/artifact-model.md`).

---

# Implementation Plan: library

> Module `library` from [`spec/capabilities.md`](../spec/capabilities.md). Spec: [`spec/library.md`](../spec/library.md). Quality bar: [`CONSTRAINTS.md`](../CONSTRAINTS.md).
> **Tasks are tracked in GitHub Issues** (Project 5) and built by the `ticket-flow` cycle. Task ids are `L1`–`L14`.
> Status: **approved 2026-10-22**; tasks created as issues #42–#55.

## Overview

Harneo's own store: import from `~/.claude`, a folder or a git URL with full provenance, literal secrets swapped for `${KEY}`, then browse, tag and collect, in screens that follow the validated prototype. Ends with the joint user checkpoint for `artifact-model` and `library`.

## Architecture Decisions

- **Core first, then screens.** L1–L7 build and test the whole store in `harneo-core` with temp data dirs; the UI tickets (L8–L13) only add thin IPC commands and screens on a finished API.
- **One thin slice before fanning out.** L2 imports a local folder end to end (preview → atomic copy → index → detail). Secrets, git, queries and tags then branch off it in parallel.
- **IPC lands with the screen that uses it.** Each UI ticket adds its own commands to `commands.rs` and wrappers to `src/lib/ipc.ts`, so no ticket ships an unused command.
- **Design is done.** The prototype and `DESIGN.md` are the reference; L14 runs `/impeccable critique` on the finished screens and the user picks what to fix, as for the app frame.
- **Shared files:** `library/mod.rs` (core API), `commands.rs` and `ipc.ts` take one-block additions per ticket; the orchestrator resolves the rebases.

## Dependency graph

```
L1 store + schema + ids ── L2 local import slice ─┬─ L3 secrets replaced
                                                  ├─ L4 git source
                                                  ├─ L5 queries ── L7 collections core (+L6)
                                                  └─ L6 tags core
L5 ── L8 Library screen ─┬─ L9 detail panel ─┬─ L11 tags UI (+L6)
                         │                   └─ L12 collections UI (+L7, L11)
                         ├─ L10 import flow (+L3, L4, L6)
                         └─ L13 palette search + git prerequisite docs
L9, L10, L11, L12 ── L14 Impeccable critique of the library screens
```

Parallel after L2: L3, L4, L5, L6. After L8: L9, L10, L13.

## Task list

Tasks are GitHub issues on Project 5; their bodies hold the acceptance criteria, verification and `Blocked by` links. The `ticket-flow` cycle picks Ready issues.

- [#42](https://github.com/ibrahimkobeissy/harneo/issues/42) L1: Library store: SQLite schema, migrations, ids
- [#43](https://github.com/ibrahimkobeissy/harneo/issues/43) L2: Import a local folder end to end (preview, atomic copy, detail, remove)
- [#44](https://github.com/ibrahimkobeissy/harneo/issues/44) L3: Literal secrets replaced by `${KEY}` in library copies
- [#45](https://github.com/ibrahimkobeissy/harneo/issues/45) L4: Git URL source: parse, shallow clone, provenance
- [#46](https://github.com/ibrahimkobeissy/harneo/issues/46) L5: Library queries: filters, sort, pages, facets
- [#47](https://github.com/ibrahimkobeissy/harneo/issues/47) L6: Tags in core
- [#48](https://github.com/ibrahimkobeissy/harneo/issues/48) L7: Collections in core
- [#49](https://github.com/ibrahimkobeissy/harneo/issues/49) L8: Library screen: cards/list, filters, sort, pages, empty state
- [#50](https://github.com/ibrahimkobeissy/harneo/issues/50) L9: Artifact detail panel
- [#51](https://github.com/ibrahimkobeissy/harneo/issues/51) L10: Import flow screens
- [#52](https://github.com/ibrahimkobeissy/harneo/issues/52) L11: Tags in the UI
- [#53](https://github.com/ibrahimkobeissy/harneo/issues/53) L12: Collections in the UI
- [#54](https://github.com/ibrahimkobeissy/harneo/issues/54) L13: Command palette finds artifacts; git as a runtime prerequisite
- [#55](https://github.com/ibrahimkobeissy/harneo/issues/55) L14: `/impeccable critique` of the library screens
  - [#57](https://github.com/ibrahimkobeissy/harneo/issues/57) C1: Library toolbar, Import button, card footer counts
  - [#58](https://github.com/ibrahimkobeissy/harneo/issues/58) C2: Import flow: diagnostics, selection, sticky footer, review polish
  - [#59](https://github.com/ibrahimkobeissy/harneo/issues/59) C3: Detail panel: Delete from library, tag menu, contrast tokens
  - [#60](https://github.com/ibrahimkobeissy/harneo/issues/60) C4: Half-width layout (≤1180px)
  - [#61](https://github.com/ibrahimkobeissy/harneo/issues/61) C5: Minor fixes: labels, dates, semantics, shortcuts

## Checkpoints

**Checkpoint: core** (after L1–L7)
- [ ] A test imports a fixture folder and a local git repo, with tags and a smart collection; no secret value anywhere in the data dir
- [ ] `check:full` passes; ratchet holds

**Checkpoint: screens** (after L8–L13)
- [ ] In `npm run tauri dev`: import a folder, browse, filter, tag, collect, remove
- [ ] Every screen matches the prototype in light and dark

**Checkpoint: joint user validation** (after L14; spec criterion 10)
- [ ] The user runs `harneo scan` on their `~/.claude`, an Addy `agent-skills` clone and the Impeccable plugin, then imports them in the app
- [ ] What looks wrong becomes a test and a fix; then `spec/capabilities.md` marks `artifact-model` and `library` done

## Spec coverage

| Spec success criterion | Tasks |
|---|---|
| 1. Import ~/.claude, folder, git with provenance | L2, L4, L10 |
| 2. Re-import shows "already in library" | L2, L10 |
| 3. Secrets replaced, value nowhere | L3, L10 |
| 4. Failed import leaves the library unchanged | L2, L4 |
| 5. Screens match the prototype; 2,000 artifacts, < 100 ms | L5, L8–L10, L14 |
| 6. Tags and collections | L6, L7, L11, L12 |
| 7. Remove | L2, L9 |
| 8. Sources never changed | L2 |
| 9. `check:full` and the ratchet | every task |
| 10. Joint user checkpoint | checkpoint |

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| `rusqlite` bundled build slows `check:task` past 3 minutes | Med | Measure in L1; it compiles once and is cached; raise it with the user rather than drop a check |
| Git behaviour differs across versions or hangs on auth | Med | `GIT_TERMINAL_PROMPT=0`, timeout, no shell; tests on local repos only |
| Secret replacement misses a format | High | The value-nowhere test reads every byte; the detector is the shared `literal_secret` helper, improved in one place |
| WebKitGTK rendering differences in the new screens | Med | Same CSS rules as the frame (no `:has()` layout); L14 critique checks the running app |
| Parallel tickets conflicting in `library/mod.rs`, `commands.rs`, `ipc.ts` | Low | Additions are separate blocks; the orchestrator rebases one ticket at a time |

## Open Questions

None.

---

# Implementation Plan: audit

> Module `audit` from [`spec/capabilities.md`](../spec/capabilities.md). Spec: [`spec/audit.md`](../spec/audit.md). Quality bar: [`CONSTRAINTS.md`](../CONSTRAINTS.md).
> **Tasks are tracked in GitHub Issues** (Project 5). Task ids `U1`–`U11`.
> Status: **pre-approved by the user (2026-10-23)** ([#70](https://github.com/ibrahimkobeissy/harneo/issues/70)); tasks created as issues #71–#81.

## Overview

Static audit of every artifact: trust score (0–100, 4 tiers), findings from rules ported from HarnessKit and extended, and reach (files, network, shell, env). Shown at import (critical candidates start excluded), in the library, the detail panel and the Security page.

## Architecture Decisions

- **Engine first (U1), then rule families in parallel (U2–U5)**: each family is its own file and adds one registry line, so parallel tickets don't collide.
- **Library integration (U6) needs only the engine**, so it runs alongside the rules; the screens (U8–U10) build on U6's stored audit and queries.
- **Ported code is attributed** in each file header plus `THIRD_PARTY_NOTICES.md` and the Apache-2.0 text.

## Dependency graph

```
U1 engine ─┬─ U2 secrets ─┐
           ├─ U3 commands ┤
           ├─ U4 mcp/supply-chain
           ├─ U5 reach + combo
           └─ U6 library integration ─┬─ U7 CLI
                                      ├─ U8 library trust UI ─┐
                                      ├─ U9 import audit UI ──┼─ U11 critique + fixes
                                      └─ U10 Security page ───┘
```

## Task list

- [#71](https://github.com/ibrahimkobeissy/harneo/issues/71) U1: Audit engine: types, score and tiers, input reader, injection rules, attribution
- [#72](https://github.com/ibrahimkobeissy/harneo/issues/72) U2: Secret and credential rules
- [#73](https://github.com/ibrahimkobeissy/harneo/issues/73) U3: Command rules: rce, dangerous-commands, safety-bypass
- [#74](https://github.com/ibrahimkobeissy/harneo/issues/74) U4: MCP and supply-chain rules
- [#75](https://github.com/ibrahimkobeissy/harneo/issues/75) U5: Reach inference and the permission-combo rule
- [#76](https://github.com/ibrahimkobeissy/harneo/issues/76) U6: Library integration: audit at preview and import, stored, re-audit, trust queries
- [#77](https://github.com/ibrahimkobeissy/harneo/issues/77) U7: `harneo audit <path> [--json]`
- [#78](https://github.com/ibrahimkobeissy/harneo/issues/78) U8: Trust in the Library: meter, column, filters, sort, detail Trust section
- [#79](https://github.com/ibrahimkobeissy/harneo/issues/79) U9: Audit in the import flow
- [#80](https://github.com/ibrahimkobeissy/harneo/issues/80) U10: Security page
- [#81](https://github.com/ibrahimkobeissy/harneo/issues/81) U11: `/impeccable critique` of the audit screens and fixes

## Checkpoints

- **Core** (after U1–U6): hostile fixtures trigger every rule; scores and tiers exact; no secret in any output; `check:full` passes.
- **Screens** (after U7–U10): trust visible across the app; critical candidates start excluded.
- **User review** (after U11): the user reviews audit together with artifact-model and library.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| False positives flood the UI (e.g. docs that mention `rm -rf`) | High | Port HarnessKit's documentation-line handling where it is sound, test the false-positive cases from the research, and keep severities conservative outside hooks/Settings |
| Secret-shaped fixtures trip gitleaks or the guard hook | Med | Build fixture secrets from fragments at test time |
| Regex cost on large packs | Low | `regex` is linear-time; patterns compiled once; a 200-artifact performance test |

---

# Implementation Plan: bundles, updates, profiles, install-engine, drift, usage, cli

Specs: `spec/bundles.md`, `spec/updates.md`, `spec/profiles.md`, `spec/install-engine.md`, `spec/drift.md`, `spec/usage.md`, `spec/cli.md`. Specs and plan **pre-approved by the user (2026-10-23)**: "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end." Tasks are tracked as GitHub issues (#87–#120) on Project 5.

## Architecture decisions

- **One linear schema:** migrations land in a fixed order, v3 `bundles` (settings + links), v4 `updates`, v5 `profiles`, v6 `install-engine` registry; each schema ticket is blocked by the previous one. `usage` keeps its own `usage.db`.
- **Process spawns in core:** `git.rs` and `bundles/claude_cli.rs` only, both with arguments, timeouts and output caps.
- **New dependencies (pre-approved in the specs):** `base64` (profiles), `similar` (install-engine; whichever ticket needs it first adds it), `tauri-plugin-opener` (install-engine), `notify` (drift).
- **One engine:** `install-engine`'s `Plan` + `apply` is used by scaffold, refit, switch, removal, drift re-apply, cleanup prune and the CLI.

## Dependency graph

```
BU1 ─┬─ BU2 ─┬─ BU4 ─┐                       US1 ─┬─ US2 ─┬─ US3 ─┐
     ├─ BU3 ─┘       ├─ BU5                       │       └─ US4 ─┼─ US5 ─ US6
     └─ UP1 ─ UP2 ─ UP3 ┘                          └─(UP1)        │
         └─ PR1 ─ PR2 ─┬─ PR3 ─┬─ PR4 ─┬─ PR6
                       │       └─ PR5 ─┘
                       └─ IE1 ─ IE2 ─┬─ IE3 ─┐
                                     ├─ IE4 ─┼─ IE7 ─┐   (IE5 → IE6, IE7)
                                     ├─ IE5 ─┴─ IE6  ├─ DR3 ─ IE8
                                     ├─ DR2 ─────────┤
                                     └─ CL1 ─ CL2, CL3
           IE4 + UP2 ─ DR1 ─ DR3, CL2;   US2 + IE4 ─ US3;   UP3 ─ BU5, US5
```

## Task list

### bundles
- [#87](https://github.com/ibrahimkobeissy/harneo/issues/87) BU1: Links between artifacts: schema v3, settings store, reference links, bundle_of
- [#88](https://github.com/ibrahimkobeissy/harneo/issues/88) BU2: Claude CLI runner and bundle/tag detection
- [#89](https://github.com/ibrahimkobeissy/harneo/issues/89) BU3: Bundles in the Library: badge, column, detail Bundle section
- [#90](https://github.com/ibrahimkobeissy/harneo/issues/90) BU4: Bundle and tag proposals at import, Find bundles, Claude CLI settings
- [#94](https://github.com/ibrahimkobeissy/harneo/issues/94) BU5: `/impeccable critique` of the bundle and update screens and fixes

### updates
- [#91](https://github.com/ibrahimkobeissy/harneo/issues/91) UP1: Update check: schema v4, copy digest, per-artifact outdated detection
- [#92](https://github.com/ibrahimkobeissy/harneo/issues/92) UP2: Update the library copy, protect local edits, update all
- [#93](https://github.com/ibrahimkobeissy/harneo/issues/93) UP3: Updates in the Library: badge, filter, callout, update and local-edit dialogs

### profiles
- [#95](https://github.com/ibrahimkobeissy/harneo/issues/95) PR1: Profiles core: schema v5, versions, CRUD, clone, from collection, resolve
- [#96](https://github.com/ibrahimkobeissy/harneo/issues/96) PR2: Profile layout and .harneo-profile.json export/import
- [#97](https://github.com/ibrahimkobeissy/harneo/issues/97) PR3: Profiles list screen, IPC, clone, export and import dialogs
- [#98](https://github.com/ibrahimkobeissy/harneo/issues/98) PR4: Profile builder: picker, cart, Settings and MCP panels, live project tree
- [#99](https://github.com/ibrahimkobeissy/harneo/issues/99) PR5: Profiles in the Library: Add to profile, Used in, Manual only, Create profile from a collection
- [#100](https://github.com/ibrahimkobeissy/harneo/issues/100) PR6: `/impeccable critique` of the profile screens and fixes

### install-engine
- [#101](https://github.com/ibrahimkobeissy/harneo/issues/101) IE1: Render a profile into project files, HARNESS.md included
- [#102](https://github.com/ibrahimkobeissy/harneo/issues/102) IE2: Apply with backups and rollback, registry schema v6, scaffold plan
- [#103](https://github.com/ibrahimkobeissy/harneo/issues/103) IE3: Refit comparison with loss flags
- [#104](https://github.com/ibrahimkobeissy/harneo/issues/104) IE4: Switch, upgrade, clean removal and backup restore
- [#105](https://github.com/ibrahimkobeissy/harneo/issues/105) IE5: Scaffold flow
- [#106](https://github.com/ibrahimkobeissy/harneo/issues/106) IE6: Refit flow
- [#107](https://github.com/ibrahimkobeissy/harneo/issues/107) IE7: Projects list and project detail: files, backups, switch, remove, open
- [#111](https://github.com/ibrahimkobeissy/harneo/issues/111) IE8: `/impeccable critique` of the scaffold, refit, projects and drift screens and fixes

### drift
- [#108](https://github.com/ibrahimkobeissy/harneo/issues/108) DR1: Drift check, diff, re-apply and promote
- [#109](https://github.com/ibrahimkobeissy/harneo/issues/109) DR2: Live rescan watcher and change events
- [#110](https://github.com/ibrahimkobeissy/harneo/issues/110) DR3: Drift in the app: pills, Drift tab, re-apply and promote

### usage
- [#112](https://github.com/ibrahimkobeissy/harneo/issues/112) US1: Session-log indexer: names only, incremental
- [#113](https://github.com/ibrahimkobeissy/harneo/issues/113) US2: Usage and status in library queries
- [#114](https://github.com/ibrahimkobeissy/harneo/issues/114) US3: Cleanup suggestions and prune through profiles and projects
- [#115](https://github.com/ibrahimkobeissy/harneo/issues/115) US4: Usage in the Library: footers, columns, Most used, Status filter, insights, detail Usage
- [#116](https://github.com/ibrahimkobeissy/harneo/issues/116) US5: Cleanup screen and Settings rows
- [#117](https://github.com/ibrahimkobeissy/harneo/issues/117) US6: `/impeccable critique` of the usage, cleanup and settings screens and fixes

### cli
- [#118](https://github.com/ibrahimkobeissy/harneo/issues/118) CL1: `harneo profiles` and `harneo scaffold`
- [#119](https://github.com/ibrahimkobeissy/harneo/issues/119) CL2: `harneo status`
- [#120](https://github.com/ibrahimkobeissy/harneo/issues/120) CL3: `harneo update`

## Checkpoints

- **Bundles + updates** (after BU4, UP3): links and proposals at import, outdated flags, local edits protected.
- **Profiles** (after PR5): profiles built, versioned, exported and imported.
- **First usable version** (after IE7): import → library → profile → scaffold or refit, with backups and clean removal (`spec/capabilities.md`).
- **Live** (after DR3, US5, CL3): drift, usage, cleanup and the CLI.
- **User review** (after the critiques BU5, PR6, IE8, US6): the user reviews every module together.

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Refit or removal loses a project file | High | Backups before every write, rollback on failure, a property test that removal restores every original byte for byte |
| `claude` CLI flags or output change | Med | Runner isolated in one file, flags checked against `claude --help`, fixture-script tests, clear errors and manual linking as the fallback |
| Claude's session-log format is not ours | Med | Tolerant parser, parse failures counted, untracked kinds say "Not tracked", synthetic fixtures only |
| Parallel tickets collide on migrations and shared files | Med | Fixed migration order through `Blocked by`; additions to shared files in their own blocks |
| App and CLI writing at once | Low | Data-dir lock plus SQLite busy timeout |

## User feedback, 2026-10-24

- [#151](https://github.com/ibrahimkobeissy/harneo/issues/151) No screen scrolls as a whole: sidebar, filters and page headers stay fixed everywhere
- [#158](https://github.com/ibrahimkobeissy/harneo/issues/158) Delete all filtered artifacts from the Library toolbar, with confirmation
- [#152](https://github.com/ibrahimkobeissy/harneo/issues/152) Tag all filtered artifacts from the Library toolbar
- [#153](https://github.com/ibrahimkobeissy/harneo/issues/153) Audit: credential paths in protective rules are not credential theft
- [#154](https://github.com/ibrahimkobeissy/harneo/issues/154) Open artifact details in a side panel on Security and Cleanup, without leaving the page
- [#155](https://github.com/ibrahimkobeissy/harneo/issues/155) Fixed action bar for the Import, Scaffold and Refit wizards
- [#156](https://github.com/ibrahimkobeissy/harneo/issues/156) Profile and project cards share one shape and size
- [#157](https://github.com/ibrahimkobeissy/harneo/issues/157) Settings: red Danger zone to reset all of Harneo's data
- [#159](https://github.com/ibrahimkobeissy/harneo/issues/159) Audit: URL query keys in code are not plaintext secrets
- [#160](https://github.com/ibrahimkobeissy/harneo/issues/160) Audit: honest guidance is not prompt injection
- [#161](https://github.com/ibrahimkobeissy/harneo/issues/161) Security audit phase 2: Claude confirms each finding, non-threats are dropped
- [#162](https://github.com/ibrahimkobeissy/harneo/issues/162) Security audit phase 2 on screen: setting, phase-1-only warning, progress and dismissed findings
- [#163](https://github.com/ibrahimkobeissy/harneo/issues/163) View an artifact's files in the app: rendered Markdown, highlighted code, or open in the default editor
