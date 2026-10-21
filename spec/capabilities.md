# Capability Map: Harneo v1

Approved 2026-10-18. This is the index of Harneo's modules and their specs. The product itself is described in [`CONTEXT.md`](../CONTEXT.md); each module spec covers one module and points back to it. Module ids are fixed: never rename them.

| Module id | Responsibility | Depends on | Spec |
|---|---|---|---|
| `app-shell` | Tauri 2 + React scaffold, Rust core library + app + CLI layout, design system, navigation, `BUILDING.md`. Also sets the project-wide conventions | — | [`app-shell.md`](app-shell.md) (approved 2026-10-19) |
| `artifact-model` | The 10 artifact types (`CONTEXT.md` §2): find them in a folder, parse them, check them; a Claude adapter behind an agent-neutral interface | — | [`artifact-model.md`](artifact-model.md) (approved 2026-10-21) |
| `library` | Harneo's own store; import from a local folder or git URL with provenance (URL, branch, subfolder, commit); browse by type and source; tags and collections; promote to library | `app-shell`, `artifact-model` | — |
| `audit` | Trust score, security findings, permission transparency | `artifact-model` | — |
| `bundles` | AI dependency detection through the `claude` CLI, plus tag suggestions in the same pass; user confirms or edits | `library` | — |
| `profiles` | Create and clone profiles, Settings template, MCP servers (as `${VAR}`), invocation mode, export/import | `library`, `bundles` | — |
| `install-engine` | Scaffold and refit: dry-run preview/diff, keep/replace/delete, backups, whole-file writes, `HARNESS.md`, project registry, switch profile, clean removal | `profiles` | — |
| `updates` | Outdated flag, update the library copy while protecting local edits | `library` | — |
| `drift` | Live rescan, drift check, re-apply or promote | `install-engine` | — |
| `usage` | Usage stats from Claude's session logs, activity graph, cleanup suggestions | `library`, `install-engine` | — |
| `cli` | `harneo scaffold`, `harneo status`, `harneo update` | `install-engine`, `updates` | — |

**Build order:** `app-shell`, `artifact-model` → `library` → `audit`, `bundles` → `profiles` → `install-engine` → `updates`, `drift`, `usage` → `cli`

## Decisions

- **Frontend:** React + Vite + TypeScript (decided 2026-10-18).
- **One engine:** a Rust core library (`harneo-core`) holds all logic. The Tauri app and the `harneo` CLI are thin layers over it.
- **UI lives with its module:** each module spec covers its core logic and its own screens. `app-shell` provides only the frame, navigation and design system.
- **`audit` is separate from `library`, and that's accepted:** artifacts imported before `audit` lands have no trust score until it does.
- **First usable version:** `app-shell` through `install-engine` (import → library → profile → scaffold or refit).

## Workflow

Each module goes through Addy Osmani's spec → plan → tasks → build steps, in build order, with the user's approval at each step:
1. `spec/<module-id>.md` is written and approved.
2. `planning-and-task-breakdown` adds the module's GitHub issues to `tasks/plan.md`.
3. The `ticket-flow` cycle builds them.
