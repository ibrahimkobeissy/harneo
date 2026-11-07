# Capability Map: Harneo v1

Approved 2026-10-18. This is the index of Harneo's modules and their specs. The product itself is described in [`CONTEXT.md`](../CONTEXT.md); each module spec covers one module and points back to it. Module ids are fixed: never rename them.

| Module id | Responsibility | Depends on | Spec |
|---|---|---|---|
| `app-shell` | Tauri 2 + React scaffold, Rust core library + app + CLI layout, design system, navigation, `BUILDING.md`. Also sets the project-wide conventions | — | [`app-shell.md`](app-shell.md) (approved 2026-10-19) |
| `artifact-model` | The 10 artifact types (`CONTEXT.md` §2): find them in a folder, parse them, check them; a Claude adapter behind an agent-neutral interface | — | [`artifact-model.md`](artifact-model.md) (approved 2026-10-21; built, user checkpoint joined with `library`) |
| `library` | Harneo's own store; import from a local folder or git URL with provenance (URL, branch, subfolder, commit); browse by type and source; tags and collections; promote to library | `app-shell`, `artifact-model` | [`library.md`](library.md) (approved 2026-10-22) |
| `audit` | Trust score, security findings, permission transparency | `artifact-model` | [`audit.md`](audit.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `bundles` | AI dependency detection through the `claude` CLI, plus tag suggestions in the same pass; user confirms or edits | `library` | [`bundles.md`](bundles.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `profiles` | Create and clone profiles, Settings template, MCP servers (as `${VAR}`), invocation mode, export/import | `library`, `bundles` | [`profiles.md`](profiles.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `install-engine` | Scaffold and refit: dry-run preview/diff, keep/replace/delete, backups, whole-file writes, `HARNESS.md`, project registry, switch profile, clean removal | `profiles` | [`install-engine.md`](install-engine.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `updates` | Outdated flag, update the library copy while protecting local edits | `library` | [`updates.md`](updates.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `drift` | Live rescan, drift check, re-apply or promote | `install-engine` | [`drift.md`](drift.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `usage` | Usage stats from Claude's session logs, activity graph, cleanup suggestions | `library`, `install-engine` | [`usage.md`](usage.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `cli` | `harneo scaffold`, `harneo status`, `harneo update` | `install-engine`, `updates` | [`cli.md`](cli.md) (pre-approved 2026-10-23; built, awaiting the user's review) |
| `harnmap` | Profile categories and templates; the Harnmap (a profile drawn as a factory by Claude, edited by the user, kept in a history, exported with the profile); Harnfit (a verdict on one new artifact against the map) | `profiles`, `bundles` | [`harnmap.md`](harnmap.md) (pre-approved 2026-11-07; built on `feat/harnmap`) |

**Build order:** `app-shell`, `artifact-model` → `library` → `audit`, `bundles` → `profiles` → `install-engine` → `updates`, `drift`, `usage` → `cli` → `harnmap`

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
