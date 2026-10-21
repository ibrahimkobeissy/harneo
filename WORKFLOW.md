# Workflow log

How Harneo is built with Addy Osmani's agent-skills and Impeccable: one line per step, in the order we ran them. Updated every time we go through a step.

1. `/spec` (step 0, capability map): split the product into 11 modules with a build order → `spec/capabilities.md`
2. `/spec`: wrote and approved the first module spec, which also sets project-wide conventions → `spec/app-shell.md`
3. `/constraints`: 4-question interview on the quality bar (what to enforce, block or warn, numbers, check budgets) → `CONSTRAINTS.md`
4. `/plan`: drafted 13 tasks in 3 phases with checkpoints; paused before creating issues, to see the design first → `tasks/plan.md` (draft)
5. `/impeccable init`: product context for the designer (users, purpose, tone, constraints) → `PRODUCT.md`
6. `/impeccable shape`: UX discovery (home, first run, navigation, library size, window, keyboard), then the visual direction round; the user took the category standard with cards, benchmarked on Vercel and Linear, and added tags and collections → the confirmed brief, `PRODUCT.md` visual preference, `CONTEXT.md` updates
7. `/impeccable` build (code-led): direction contract written, prototype built and click-tested, finish review (fix → 2 verdict rounds, one small item left open), documenter → `design/prototype/`, `DESIGN.md`, `.impeccable/design.json`
8. Prototype round 2 (user feedback, within the established design): pagination, sortable columns, Security page, settings template contents, smart vs manual collections, back link in bundles, filter column scroll, shortcuts button; checked against `DESIGN.md` by Impeccable's detector → `design/prototype/`
9. `/impeccable` documenter (update): `DESIGN.md` extended with the round-2 components (pager, sortable tables, rule chips, security tiles, back link) and the Every Column Sorts rule; three small build drifts fixed → `DESIGN.md`, `.impeccable/design.json`
10. `/plan` (finished): plan revised after the validated prototype and approved; one GitHub issue per task with `Blocked by` links, on the board (T1 Ready, the rest Backlog) → issues #11–#23, `tasks/plan.md` as their ordered index
11. `ticket-flow` build cycles: Sonnet subagents built #11–#18 and #21–#22 in worktrees; the orchestrator validated, fixed and merged each to `main` → the Tauri app, core + CLI, design tokens, and the `check:fast` / `check:task` gates
12. `/impeccable critique` on the styled frame (#22): design review + detector, 24/40; the user chose to fix all 5 priority issues and the minor items before merging → `.impeccable/critique/`, fixes in #22
13. `ticket-flow` build cycle #19: `check:full` (full tests, dependency scan that fails at high or above, semgrep, coverage ratchet) and the GitHub Actions workflow merged; the first CI run was refused for account billing → `scripts/check-full.sh`, `scripts/check-deps.sh`, `.github/workflows/check.yml`
14. `/constraints ratchet` (#20): project coverage recorded as the baseline, Rust 74.58% and TypeScript 95.17% on 8718bed; the ratchet now holds them in `check:full` → `CONSTRAINTS.md` "Measured, not yet enforced"
15. `ticket-flow` build cycle #25: setup scripts for building from source (`--check`, `--dev`, per-OS package lists, 33 stubbed tests in `check:fast`) → `scripts/setup.sh`, `scripts/setup.ps1`, `scripts/setup.test.sh`
16. `ticket-flow` build cycle #23: `BUILDING.md` (setup script, per-OS prerequisites, build and run, gate tools), README and `CLAUDE.md` commands; release build verified (`target/release/harneo-app`, `target/release/harneo`) → `BUILDING.md`, `README.md`, `CLAUDE.md`
17. `/spec` artifact-model: spec drafted from the user's answers (all folder shapes, structured references only, `harneo scan` CLI, literal secrets as warnings) → `spec/artifact-model.md`, approved by the user
18. `/plan` artifact-model: 12 tasks (A1–A12) with dependency graph, three checkpoints and risks, awaiting approval → `tasks/plan.md` (artifact-model section)
