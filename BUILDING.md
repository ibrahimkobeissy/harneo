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

Installers are not built (`bundle.active` is `false` in `src-tauri/tauri.conf.json`). The release build writes the app binary to `target/release/harneo-app` (`harneo-app.exe` on Windows), with the frontend embedded from `dist/`. The CLI binary is `target/release/harneo`. The app and the CLI share one library folder; `harneo paths` prints where it is.

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

| Command              | When                                                  | Budget  | Runs                                                                                                                  |
| -------------------- | ----------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------- |
| `npm run check:fast` | After each edit                                       | ~15 s   | Types, TS lint and format on changed files, `cargo check`, secrets, floor rules                                       |
| `npm run check:task` | Before a ticket is reported done                      | 3 min   | `check:fast` + Rust format and Clippy, related tests, changed-line coverage, architecture rules                       |
| `npm run check:full` | Before every merge to `main`; CI on every push        | none    | `check:task` + the full test suites, dependency and code scanning, the coverage ratchet                               |

Every rule blocks. `CONSTRAINTS.md` is the source of truth for what each check enforces.
