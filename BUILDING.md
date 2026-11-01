# Building Harneo

Harneo is distributed as source only: clone the repo and build it for your own OS. The maintainer builds and tests on Linux only. Windows and macOS are untested by the maintainer; please report problems.

## Quick start

```bash
./scripts/setup.sh           # check what is missing, then install it after a confirmation
npm ci
npm run tauri dev
```

On Windows run `scripts\setup.ps1` instead (`-Check`, `-Dev` and `-Yes` mirror the flags below).

`scripts/setup.sh` options:

| Flag      | Effect                                                                                                              |
| --------- | ------------------------------------------------------------------------------------------------------------------- |
| `--check` | Install nothing. Print what is missing and exit 1 if anything is.                                                   |
| `--dev`   | Also install the contributor gate tools (see [Contributor tools](#contributor-tools)).                              |
| `--yes`   | Skip the confirmation (`HARNEO_SETUP_YES=1` does the same). Steps that need administrator rights still use `sudo`. |

The script shows every command before it runs anything. The rest of this page lists what it installs, in case you prefer to do it by hand.

## Prerequisites

### Linux (tested)

Tauri 2 needs WebKitGTK 4.1 and a few system libraries. Package names per distro:

- **Arch (pacman):** `webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool`
- **Debian, Ubuntu (apt):** `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
- **Fedora (dnf):** `webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel`, plus the group `C Development Tools and Libraries` (`sudo dnf group install "C Development Tools and Libraries"`)

### Windows (untested by the maintainer)

- WebView2 runtime (preinstalled on Windows 11).
- MSVC build tools: Visual Studio 2022 Build Tools with the "Desktop development with C++" workload.

### macOS (untested by the maintainer)

- Xcode command line tools: `xcode-select --install`.
- Homebrew, which `setup.sh` uses for the rest.
- Homebrew installs `node@22` and `rustup` keg-only, so they are not on your `PATH` until you add `$(brew --prefix node@22)/bin`, `$(brew --prefix rustup)/bin` and `~/.cargo/bin` (`setup.sh` prints the line to add).

### Git (all platforms)

Install `git` (pacman, apt, dnf and winget `Git.Git` use that name; on macOS the Xcode command line tools provide it). Harneo runs `git` to import artifacts from a git URL, and you need it to clone the repo.

### Rust

Install Rust through [rustup](https://rustup.rs). `rust-toolchain.toml` pins the channel (stable) and the components (rustfmt, clippy, llvm-tools-preview); rustup installs them on the first `cargo` run in the repo.

### Node

Node.js 22 or newer (the floor `setup.sh` checks), with npm.

## Install dependencies

```bash
npm ci
```

Rust dependencies are fetched and built on the first `cargo` or `tauri` run.

## Build and run

| Goal                       | Command                                                                  |
| -------------------------- | ------------------------------------------------------------------------ |
| Run the app, hot reload    | `npm run tauri dev`                                                      |
| Release build, no bundle   | `npm run tauri build -- --no-bundle`                                     |
| Run the CLI                | `cargo run -p harneo-cli -- --version`, or `-- paths` for the data folder |
| Build the CLI in release   | `cargo build --release -p harneo-cli`                                    |

Installers are not built (`bundle.active` is `false` in `src-tauri/tauri.conf.json`). The release build writes the app binary to `target/release/harneo-app` (`harneo-app.exe` on Windows), with the frontend embedded from `dist/`. The CLI binary is `target/release/harneo`. The app and the CLI share one library folder; `harneo paths` prints where it is. The README's [Install](README.md#install) section has the per-OS steps to put both on your `PATH`.

### Bundles (optional)

Untested by the maintainer. To build an OS package instead of a plain binary, turn bundling on for one build and name the format:

| OS | Command | Output |
| -- | ------- | ------ |
| macOS | `npm run tauri build -- --bundles app --config '{"bundle":{"active":true}}'` | `target/release/bundle/macos/Harneo.app` (unsigned: right-click → Open the first time) |
| Windows | `npm run tauri build -- --bundles nsis --config '{"bundle":{"active":true}}'` | `target\release\bundle\nsis\*.exe` installer |
| Linux | `npm run tauri build -- --bundles deb --config '{"bundle":{"active":true}}'` (or `rpm`, `appimage`) | `target/release/bundle/<format>/` |

In PowerShell, quote the JSON as `--config '{\"bundle\":{\"active\":true}}'`.

## Error reporting (optional)

Harneo can send errors to a Sentry project of your own. Nothing is sent until you set a DSN; once it is set, reporting is on unless you turn it off in Settings. No DSN is committed or built into Harneo.

1. Create a project in Sentry and copy its DSN. Before starting Harneo or the CLI, `export HARNEO_SENTRY_DSN=<your DSN>` (keep it out of committed files). Without it, the Settings switch is disabled and nothing is ever sent.
2. That's it: with the DSN set, Settings → Diagnostics → "Send error reports to Sentry" is on by default. Turn it off there to stop reporting; that choice is kept even with the DSN set. The `harneo` command follows the same switch. Restart Harneo for a change to take effect.

Only errors are reported (no performance tracing, session replay, profiling or sessions), at most 20 per run and each distinct error once. Before sending, the home folder becomes `~`, request bodies, extra data and breadcrumb details are dropped, messages are cut to 500 characters, and any event containing a token or key shape is not sent. File contents, audit evidence and Claude prompts are never attached.

## Contributor tools

You do not need these to build Harneo. They run the quality gates that `CONSTRAINTS.md` defines, and `./scripts/setup.sh --dev` installs them:

- `gitleaks` and `osv-scanner`: secret and dependency scanning.
- `semgrep`: code scanning.
- `diff-cover` and `cargo-llvm-cov`: coverage of changed lines.
- `jq`: used by the check scripts.

The maintainer manages them with [mise](https://mise.jdx.dev). If a shell does not see them, load the mise environment first:

```bash
eval "$(mise env -s bash)"
```

## Checks

| Command              | When                                                     | Budget | Runs                                                                                            |
| -------------------- | -------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------- |
| `npm run check:fast` | After each edit                                          | ~15 s  | Types, TS lint and format on changed files, `cargo check`, secrets, floor rules                 |
| `npm run check:task` | Before a ticket is reported done                         | 3 min  | `check:fast` + Rust format and Clippy, related tests, changed-line coverage, architecture rules |
| `npm run check:full` | Before every merge to `main`; CI on `release_x.y.z` tags | none   | `check:task` + the full test suites, dependency and code scanning, the coverage ratchet         |

Every rule blocks. `CONSTRAINTS.md` is the source of truth for what each check enforces.
