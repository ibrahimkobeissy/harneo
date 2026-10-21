# Constraints

Harneo's quality bar. Code is written by Sonnet subagents and validated by an orchestrator session (see the `ticket-flow` skill), so the bar is written down and checked by commands, not left to whoever reads the diff.

Last reviewed: 2026-10-19 by @ibrahimkobeissy. Decided through Addy Osmani's `constraint-driven-development` interview ([#5](https://github.com/ibrahimkobeissy/harneo/issues/5)).

**Every rule blocks.** A subagent doesn't report a ticket done while a check fails, and the orchestrator doesn't merge to `main` until `check:full` passes. When a check fails during validation, the orchestrator fixes the code; it never relaxes the check.

**Status:** no code exists yet. The tools, configs and scripts named below are installed by the `app-shell` tickets (see `spec/app-shell.md`). Until then, the Floor applies to every change by review.

## Floor (always enforced)

| Rule | Why | Checked by |
|---|---|---|
| No new suppressions: `@ts-ignore`, `@ts-nocheck`, `@ts-expect-error`, `eslint-disable*`, Rust `#[allow(…)]` / `#![allow(…)]` | A silenced check is a failing check nobody sees | `scripts/check-floor.sh` (added lines of the diff against `main`) |
| No unfinished stubs: `todo!()`, `unimplemented!()`, `throw new Error("Not implemented")`, empty `catch {}` | A stub passes the build and fails the user | `scripts/check-floor.sh` |
| No skipped, focused or deleted tests (`it.skip`, `.only`, `#[ignore]`, removed test cases) unless the commit message has a `Test-change: <reason>` line | Removing a test is the cheapest way to get green | `scripts/check-floor.sh` (reads the diff and the commit messages) |
| No `unwrap()` / `expect()` outside tests | A panic in the core kills the app or the CLI mid-write, which matters for backups | Clippy: `unwrap_used` and `expect_used` set to deny in `[workspace.lints]`, with `allow-unwrap-in-tests` / `allow-expect-in-tests` in `clippy.toml` |
| No `unsafe` Rust | Harneo reads and writes files; it has no need for `unsafe` | `unsafe_code = "forbid"` in `[workspace.lints]` |
| No secrets in source | Harneo handles MCP configs, where secrets must exist only as `${VAR}` | `gitleaks dir . --redact --no-banner` (always `--redact`: the matched value must never reach a transcript) |
| This file doesn't get weakened to make a change pass | Otherwise every rule above is optional | Subagents never edit `CONSTRAINTS.md`; the orchestrator treats any diff to it as needing the user's approval |

## Enforced with numbers

| Dimension | Rule | Why this number | Checked by | Runs at |
|---|---|---|---|---|
| Types (TS) | Zero errors | Strict mode is the spec's contract | `tsc --noEmit` | `check:fast` |
| Lint and format (TS) | Zero errors on changed files | One style, no review time spent on it | `eslint <changed files>` + `prettier --check <changed files>` | `check:fast` |
| Compile (Rust) | Zero errors | Cheapest Rust signal; incremental, so it fits the edit loop | `cargo check --workspace` | `check:fast` |
| Lint and format (Rust) | Zero warnings | Clippy finds real bugs; `-D warnings` keeps warnings from piling up | `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings` | `check:task` |
| Tests | All pass | A ticket is done only when its behaviour is proven | `vitest related --run <changed files>` + `cargo test -p <touched crates>` (`check:task`); `vitest run` + `cargo test --workspace` (`check:full`) | `check:task`, `check:full` |
| Coverage of changed lines | ≥ 80%, Rust and TS | High enough that every real function is tested, low enough that nobody writes empty tests to hit a number. Changed lines only, because that's a number the change can actually move | `vitest run --coverage` (lcov) + `cargo llvm-cov --lcov`, then `diff-cover <lcov files> --compare-branch=main --fail-under=80` | `check:task` |
| Architecture (TS) | Zero violations: only `src/lib/ipc.ts` imports `invoke` from `@tauri-apps/api`; `src/features/<a>` never imports `src/features/<b>`; `src/ui/` imports neither `src/features/` nor `src/app/` | Keeps the IPC boundary in one file and screens independent, as `spec/app-shell.md` requires | `depcruise --validate .dependency-cruiser.cjs src` | `check:task` |
| Architecture (Rust) | `harneo-core` depends on none of `tauri*`, `clap`, `harneo-app`, `harneo-cli` | "Logic lives in core" is what keeps the app and the CLI on one engine | `scripts/check-arch.sh` (inspects `cargo tree -p harneo-core -e normal`) | `check:task` |
| Security: dependencies | Nothing at high or above | Harneo imports untrusted artifacts and writes into the user's projects; a vulnerable dependency is a way in | `scripts/check-deps.sh`: osv-scanner on `Cargo.lock` and `package-lock.json`; fails at CVSS 7.0 or above, prints lower and unscored advisories as warnings | `check:full` |
| Security: code | No findings at ERROR severity | Same reason; Semgrep catches risky patterns like shell injection and path traversal | `semgrep scan --config p/default --config p/rust --config p/typescript --severity ERROR --error` | `check:full` |

## Where each check runs

| Script | When | Budget | Contents |
|---|---|---|---|
| `npm run check:fast` | After each edit | ~15 s | Types, TS lint and format on changed files, `cargo check`, secrets, floor |
| `npm run check:task` | When a subagent thinks its ticket is done | ≤ 3 min | `check:fast` + Rust format and Clippy, related tests, changed-line coverage, architecture (TS and Rust) |
| `npm run check:full` | The orchestrator, before every merge to `main`; CI on every push | No limit | `check:task` + the full test suites, dependency and code scanning, the coverage ratchet |

The task budget is 3 minutes instead of the usual 90 seconds because Rust compile times don't fit in 90. This file is the source of truth for the commands; the `package.json` scripts mirror it, and if they drift, this file wins.

## Measured, not yet enforced

| Metric | Today | Direction | Checked by |
|---|---|---|---|
| Project coverage, Rust | 74.58% (2026-10-21, commit 8718bed) | Must not fall | `scripts/coverage-ratchet.sh` in `check:full` |
| Project coverage, TypeScript | 95.17% (2026-10-21, commit 8718bed) | Must not fall | `scripts/coverage-ratchet.sh` in `check:full` |

## Dropped on purpose

- **Performance budgets:** Lighthouse and the other usual tools need a web page at a URL; Harneo is a desktop app.
- **Accessibility:** dropped by the user's choice (2026-10-19).

## Exceptions

| ID | Rule | Path | Reason | Owner | Expires |
|---|---|---|---|---|---|
| — | none | — | — | — | — |
