# Harneo

Harneo is a harness manager for Claude Code: a Tauri desktop app that keeps a library of AI Artifacts, composes profiles from it, and scaffolds or refits projects from a profile. **Read `CONTEXT.md` before designing or building anything.** It is the product source of truth: artifacts, user journey, feature decisions and constraints.

## Status
- The repo is empty apart from `CONTEXT.md`, `LICENSE` (MIT) and `README.md`. No code, no build commands yet. Add them here once the Tauri scaffold exists.
- Stack decided: Tauri 2. The frontend framework is not decided yet.
- Cross-platform code, built and tested on Linux only. Distribution is source only (see `CONTEXT.md` §4–5).

## Issue tracker
- Tasks are tracked in **GitHub Issues** for `ibrahimkobeissy/harneo`, using the `gh` CLI. Do not write `tasks/todo.md`.
- One issue per task: acceptance criteria and verification steps in the issue body; dependencies as "Blocked by #N".
- Add every new issue to GitHub Project 5 ("Harneo", https://github.com/users/ibrahimkobeissy/projects/5): `gh project item-add 5 --owner ibrahimkobeissy --url <issue-url>`.
- `tasks/plan.md` keeps only the ordered list of issue links, never a duplicate checklist.
