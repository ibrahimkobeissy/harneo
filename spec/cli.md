# Spec: cli

> Module `cli` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 *Everywhere — the CLI companion*: `harneo scaffold --profile heavy ./my-api`, `harneo status`, `harneo update`, "the same engine as the app, for terminals and scripts". The prototype has no CLI screens.
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`install-engine.md`](install-engine.md), [`drift.md`](drift.md), [`updates.md`](updates.md) and [`profiles.md`](profiles.md). The `harneo` binary already has `paths`, `scan` and `audit` (`crates/harneo-cli/`).

## Objective

Give terminals and scripts the three everyday actions without opening the app, on the same data folder and engine: scaffold a project from a profile, see the status of registered projects, and update library copies. Everything the app protects stays protected: dry run first, backups, no silent overwrite of local edits.

**User:** the builder in a terminal or a setup script. Success: `harneo scaffold --profile heavy ./my-api` prints the files it will write, asks, writes them and registers the project; `harneo status` lists every project with its drift; `harneo update --check` lists what is outdated.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **Scaffold only, no refit in the CLI:** refit needs per-item choices; `scaffold` on a folder with a harness exits with "…has a harness; refit it in the app". `--dry-run` prints the plan; without `--yes`, an interactive terminal is asked to confirm and a non-interactive one gets exit 2 ("pass --yes").
2. **Profiles are named by name (case-insensitive) or id**, at their current version unless `--version N`.
3. **`update` never resolves local edits:** edited artifacts are skipped and listed ("update it in the app to choose"), like the app's *Update all*.
4. **The app and CLI can run together:** SQLite's busy timeout plus the data-dir lock from `install-engine` around every write; a second writer waits up to 10 s then exits with "Harneo is busy".
5. **Exit codes:** 0 done; 1 error; 2 confirmation needed; `status --exit-code` returns 3 when any project drifted (for scripts).

### Scope

**CLI (`crates/harneo-cli/src/`)**
- `harneo profiles [--json]`: name, version, artifact count, projects.
- `harneo scaffold --profile <name|id> [--version N] <path> [--dry-run] [--yes] [--json] [--no-plugins]`: plan (each file with *new*), confirm, apply, then "Scaffolded <path> with <profile> vN: N files, registered.", then installs the profile's plugins from their marketplaces and prints one `plugin <id> installed | already installed | NOT installed: <why>` line each (`spec/install-engine.md` 4b); `--no-plugins` skips that.
- `harneo status [<path>] [--json] [--exit-code]`: without a path, every registered project with profile, version, and *In sync* / *Drift · N files* / *Profile vN available* / *Library has newer copies* / *Missing*; with a path, that project's drifted files with their change and `+N −M`.
- `harneo update [--check] [<artifact name|id>…] [--json]`: `--check` runs the check and lists outdated (from → to, local edits) and per-source errors; without `--check`, checks then updates every outdated artifact without local edits (or only the named ones), listing skipped ones.
- Human output follows `scan`/`audit` (`render()` building a string, plain text, no colour when not a TTY); `--json` prints the core types pretty-printed.

**Core:** nothing new beyond what the modules expose; any helper the CLI needs goes into the owning module.

**Out of scope:** refit, switch, removal, promote and cleanup from the CLI (app only in v1); shell completions; a long-running watch mode.

## Tech Stack

No new dependency. TTY detection uses `std::io::IsTerminal`.

## Commands

```bash
cargo test -p harneo-cli
cargo run -p harneo-cli -- scaffold --profile heavy ./tmp-app --dry-run
```

## Project Structure

```
crates/harneo-cli/src/
  profiles.rs, scaffold.rs, status.rs, update.rs   (one file per subcommand, like scan.rs / audit.rs)
crates/harneo-cli/tests/cli.rs                      (assert_cmd; temp data dir through HARNEO_DATA_DIR)
```

`HARNEO_DATA_DIR` (env) overrides the data folder for the CLI and tests; the app ignores it unless set.

## Code Style

```rust
/// `harneo scaffold`: plan, confirm, apply. Never writes without --yes or a confirmation.
pub fn run(args: &ScaffoldArgs, out: &mut impl Write, confirm: impl FnOnce(&Plan) -> io::Result<bool>) -> anyhow::Result<ExitCode> { … }
```

## Testing Strategy

- assert_cmd with a temp data dir (`HARNEO_DATA_DIR`) seeded through the core API: profiles listing; scaffold dry-run, `--yes`, refused harness, non-interactive without `--yes` (exit 2); status with drift (exit 3 with `--exit-code`) and a missing project; update `--check` against a local git source, update skipping an edited artifact; human and JSON output.
- The data-dir lock: a held lock makes a write wait and then fail with "Harneo is busy".

## Boundaries

- **Always:** dry run available; confirmation before writes; same engine as the app.
- **Ask first:** adding refit or removal to the CLI.
- **Never:** overwrite local edits; write without `--yes` in a non-interactive shell.

## Success Criteria

1. `harneo scaffold` writes exactly what `--dry-run` prints and registers the project; the app shows it.
2. `harneo status` matches the app's project pills.
3. `harneo update` updates only unedited outdated artifacts and lists the rest.
4. The app and CLI used together never corrupt the data folder (lock test).
5. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: no refit in the CLI; exit code 3 for drift behind a flag.
