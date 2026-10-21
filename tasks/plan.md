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
