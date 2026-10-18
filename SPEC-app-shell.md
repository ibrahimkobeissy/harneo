# Spec: app-shell

> Module `app-shell` of the [capability map](CAPABILITIES.md). Status: **draft, awaiting approval.**
> Product context: [`CONTEXT.md`](CONTEXT.md), especially §4 (distribution, `BUILDING.md`) and §5 (build constraints).
> **The "Conventions" half of this spec applies to the whole project.** Later module specs follow it and only record exceptions.

## Objective

Give Harneo a working base on which each later module adds its own core logic and screens:
- a Tauri 2 desktop app with a React frontend that builds and runs on Linux from source;
- a Rust core library holding all logic, shared by the app and a `harneo` CLI;
- an app frame with navigation and a design system;
- one command that runs every check;
- a CI run on Linux;
- a build guide for each OS.

**User of this module:** the developer building Harneo, including the Sonnet subagents in the `ticket-flow` cycle. It ships no end-user features. Its job is to make every later ticket quick to build and easy to check.

### Scope

- Cargo workspace with three crates: `harneo-core` (library), `harneo-app` (Tauri), `harneo-cli` (binary `harneo`).
- `harneo-core`:
  - `version()`;
  - data-dir resolution (`paths`);
  - the shared error type.
  No product logic yet.
- `harneo-app`:
  - Tauri 2 window;
  - one IPC command, `app_info` (returns version + data dir);
  - TypeScript types generated from the Rust types.
- `harneo-cli`: `harneo --version` and `harneo paths` (prints the data dir), both read from `harneo-core`.
- Frontend:
  - React app frame with a sidebar for **Library**, **Profiles** and **Projects** (empty placeholder screens);
  - an **About** panel showing the version and data dir from `app_info`;
  - light and dark themes that follow the OS setting.
- Design system: CSS custom-property tokens (color, type, spacing, radius) and a few base components (Button, Panel, EmptyState), built with the Impeccable skill.
- Tooling and docs:
  - the `npm run check` script;
  - a GitHub Actions workflow (Linux) running it;
  - `BUILDING.md`, plus build commands in `README.md` and `CLAUDE.md`;
  - the Impeccable plugin enabled for this repo in `.claude/settings.json` (from the `pbakaus/impeccable` marketplace).

**Out of scope:** any artifact, library or profile logic; installers and app bundles (distribution is source only); macOS/Windows testing; end-to-end tests of the real window (see Open Questions).

## Tech Stack

| Layer | Choice |
|---|---|
| Desktop | Tauri 2 (latest 2.x) |
| Core / CLI | Rust stable (pinned by `rust-toolchain.toml`), edition 2021; `serde`, `thiserror`; `clap` (CLI); `dirs` (data dir); `ts-rs` (TS bindings) |
| Frontend | React 19, TypeScript (strict), Vite, React Router (screens), `@tauri-apps/api` |
| Styling | Plain CSS: global tokens in `src/ui/tokens.css` + CSS Modules per component. No CSS framework, no UI kit (keeps WebKitGTK, the weakest of the three engines, safe and the design in our hands) |
| Tests | `cargo test`; Vitest + React Testing Library + jsdom; Tauri's `mockIPC` for the frontend |
| Lint / format | `cargo fmt`, `cargo clippy -D warnings`; ESLint (flat config) + Prettier; `tsc --noEmit` |
| Package manager | npm (fewest prerequisites for people building from source). Node.js: current active LTS |

Versions are the latest stable releases when the scaffold is created, locked by `Cargo.lock` and `package-lock.json`. Both lockfiles are committed.

## Commands

```bash
npm install                          # once: JS deps (Rust deps build on first run)
npm run tauri dev                    # run the app with hot reload
npm run tauri build -- --no-bundle   # release binary only, no installer (source-only distribution)
cargo run -p harneo-cli -- --version # run the CLI
npm run check                        # EVERYTHING below; the one command every ticket must pass

# what `npm run check` runs, in order:
npm run lint          # eslint . && prettier --check .
npm run typecheck     # tsc --noEmit
npm test              # vitest run
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`npm run fix` applies `eslint --fix`, `prettier --write .` and `cargo fmt --all`.

## Project Structure

```
Cargo.toml              → workspace root (members: crates/*, src-tauri)
rust-toolchain.toml     → pins the stable toolchain
crates/
  harneo-core/          → ALL logic. No Tauri, no UI, no CLI dependencies
    src/lib.rs          → public API, one `pub mod` per module id (artifact, library, …)
    src/paths.rs        → data-dir resolution
    src/error.rs        → `Error` enum (thiserror), `Result<T>`
    tests/              → integration tests (temp dirs, real files)
  harneo-cli/           → binary `harneo`: argument parsing + printing, calls harneo-core
src-tauri/              → crate `harneo-app`: Tauri setup + thin `#[tauri::command]`s calling harneo-core
  src/commands.rs       → every IPC command, grouped by module
  tauri.conf.json
src/                    → React frontend
  main.tsx, App.tsx     → entry, router, app frame
  app/                  → frame: sidebar, layout, About panel
  features/<module-id>/ → one folder per module's screens (library/, profiles/, projects/ …)
  ui/                   → design system: tokens.css + base components
  lib/ipc.ts            → the ONLY place that calls `invoke`; typed wrappers per command
  bindings/             → TS types generated by ts-rs (committed, never hand-edited)
  test/setup.ts         → Vitest setup (jsdom, mockIPC helpers)
BUILDING.md             → build-from-source guide, per OS
.github/workflows/check.yml → `npm run check` on ubuntu-latest
```

Rules:
- **Logic lives in `harneo-core`.** `src-tauri` and `harneo-cli` only convert arguments and results. Anything testable without a window or terminal goes in core.
- **One module id = one folder:** `harneo-core/src/<module>/` and `src/features/<module>/`.
- Frontend tests sit next to their files (`Foo.test.tsx`). Rust unit tests go in `#[cfg(test)] mod tests`, integration tests in `crates/harneo-core/tests/`.

## Code Style

```rust
// crates/harneo-core/src/paths.rs
/// Harneo's data folder: the library, profiles and backups live here.
/// Same rule as Tauri's `app_data_dir`, so the app and the CLI share one library.
pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|base| base.join(APP_ID))
        .ok_or(Error::NoDataDir)
}
```

```rust
// src-tauri/src/commands.rs: thin, no logic
#[tauri::command]
pub fn app_info() -> Result<AppInfo, String> {
    Ok(AppInfo { version: harneo_core::version().into(), data_dir: harneo_core::paths::data_dir().map_err(|e| e.to_string())? })
}
```

```tsx
// src/app/AboutPanel.tsx
export function AboutPanel() {
  const info = useAppInfo(); // wraps ipc.appInfo()
  if (!info) return <EmptyState title="Loading…" />;
  return (
    <Panel title="About Harneo">
      <p>Version {info.version}</p>
      <p className={styles.path}>{info.dataDir}</p>
    </Panel>
  );
}
```

Conventions:
- **Rust:** `rustfmt` defaults; `thiserror` errors in core and `anyhow` only in the CLI; no `unwrap()`/`expect()` outside tests; doc comments on public items say *why*, not *what*.
- **TypeScript:** strict mode; function components and hooks; named exports; no `any`; components in `PascalCase.tsx`, hooks in `useThing.ts`.
- **IPC:** a Rust type that crosses IPC derives `Serialize` + `ts_rs::TS`, and the frontend imports it from `src/bindings/`. The frontend never calls `invoke` outside `src/lib/ipc.ts`.
- **CSS:** only token variables for color, spacing, type and radius (no raw hex or px in components); only features that WebKitGTK supports (no `:has()`-dependent layout or other bleeding-edge CSS without a fallback).
- **Paths:** never hard-code OS paths. Core resolves them with `dirs`; the frontend gets them over IPC.
- Commits: `<what changed> (#N)` per `ticket-flow`.

## Testing Strategy

| Level | Tool | Covers |
|---|---|---|
| Core unit | `cargo test` (in-module) | pure functions: parsing, rules, path building |
| Core integration | `cargo test` (`crates/harneo-core/tests/`) with temp dirs | anything that reads or writes files. Every write path gets a test, which matters for the backup/no-merge rules later |
| CLI | `cargo test -p harneo-cli` (`assert_cmd`) | arguments → output, for each command |
| Frontend | Vitest + React Testing Library, IPC mocked with `mockIPC` | screens render, user flows, error and empty states |

- Write tests first (Addy's `test-driven-development`). Every bug fix starts with a failing test.
- No coverage percentage target. The rule: every public core function and every IPC command has at least one test.
- `npm run check` must pass before every commit to `main`. CI runs it again on every push.

**For app-shell itself:**
- `paths::data_dir()` ends in the app id;
- `version()` matches `CARGO_PKG_VERSION`;
- `harneo --version` and `harneo paths` print the expected output;
- the frame renders all three sidebar entries and switches screens;
- the About panel shows mocked `app_info` data.

## Boundaries

- **Always:**
  - run `npm run check` before committing;
  - put logic in `harneo-core`;
  - resolve paths through core;
  - keep secrets as `${VAR}` placeholders;
  - back up before any write to a user project (from `install-engine` on);
  - follow `ticket-flow`.
- **Ask first:**
  - adding a dependency (Rust or npm) that isn't in this spec;
  - changing CI or `.claude/` config;
  - changing a convention in this spec;
  - any change to a module boundary in `CAPABILITIES.md`.
- **Never:**
  - merge content into an existing user file;
  - write outside the target project and Harneo's data folder;
  - touch `CLAUDE.local.md` or `settings.local.json`;
  - hard-code OS paths;
  - put OS-specific shell calls in core;
  - commit secrets;
  - skip or delete a failing test to get green;
  - build installers or sign code.

## Success Criteria

1. On a clean Linux machine, following `BUILDING.md` alone, `npm install && npm run tauri dev` opens the Harneo window.
2. The window shows a sidebar with Library, Profiles and Projects. Clicking each one shows its placeholder screen.
3. The About panel shows the version and data dir received from `app_info`.
4. The theme follows the OS light/dark setting, and every color comes from tokens.
5. `cargo run -p harneo-cli -- --version` prints the same version the About panel shows, and `harneo paths` prints the same data dir. That proves one shared core.
6. `npm run check` passes locally, and the GitHub Actions workflow passes on `main`.
7. `npm run tauri build -- --no-bundle` produces a runnable binary and no installer.
8. `BUILDING.md` lists prerequisites for Linux (WebKitGTK and the other Tauri system packages), Windows (WebView2, MSVC build tools) and macOS (Xcode command line tools), with Windows and macOS marked "untested by the maintainer".
9. `CLAUDE.md` and `README.md` list the commands, and the Impeccable plugin is enabled in `.claude/settings.json`.

## Open Questions

1. **App id:** `io.github.ibrahimkobeissy.harneo`. Tauri requires a reverse-domain identifier, and this one is based on your GitHub account since there's no harneo domain. It fixes the data dir path, so it's hard to change later. OK?
2. **End-to-end tests of the real window** (`tauri-driver` + WebKitWebDriver on Linux) are left out of the shell: they're heavy to set up and there's nothing to test yet. Should they be added when the first real flow lands (`library` import), or never, relying on frontend tests with mocked IPC plus manual runs?
3. **Impeccable:** enable it as a plugin from the `pbakaus/impeccable` marketplace in `.claude/settings.json`, the same way Addy's plugin is set up. OK?
