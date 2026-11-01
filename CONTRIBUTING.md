# Contributing to Harneo

Thanks for your interest. Harneo is a personal tool shared openly, maintained by one person, so the most useful contributions are focused and well described.

## Ways to help

- **Platform reports.** macOS and Windows are untested by the maintainer. A report that says "built and ran on macOS 15 / Windows 11, here is what worked and what didn't" is valuable even with no fix attached. Use the *Platform report* issue template.
- **Bug reports.** Use the *Bug report* template: what you did, what you expected, what happened, your OS, and the output of `harneo --version`.
- **Feature ideas.** Open a *Feature request* first and wait for a reply before writing code. [CONTEXT.md](CONTEXT.md) lists what Harneo deliberately does not do (no in-app editor, no marketplace browsing, Claude Code only in v1), so check there.
- **Pull requests.** Welcome for issues that have been discussed. Small and focused beats large.

## Development setup

Follow [BUILDING.md](BUILDING.md), then install the contributor gate tools:

```bash
./scripts/setup.sh --dev       # Windows: scripts\setup.ps1 -Dev
npm ci
npm run tauri dev
```

Read these before writing code:

- [CONTEXT.md](CONTEXT.md): the product source of truth (artifacts, user journey, decisions, constraints).
- [CONSTRAINTS.md](CONSTRAINTS.md): the quality bar. Every rule blocks; please don't weaken a rule to make a change pass.
- `spec/`: one spec per module. A change in behaviour should match, or update, its spec.
- [DESIGN.md](DESIGN.md): the design system, for any UI change.

## Checks

| Command              | When                     |
| -------------------- | ------------------------ |
| `npm run check:fast` | after each edit          |
| `npm run check:task` | before opening a PR      |
| `npm run check:full` | the maintainer runs it before merging |

New behaviour comes with tests (Vitest for `src/`, `cargo test` for the Rust crates). Bug fixes come with a test that failed before the fix.

## Pull requests

1. Link the issue the PR resolves (`Fixes #N`).
2. Keep one change per PR, with no unrelated formatting or refactors.
3. Say how you verified it, and on which OS.
4. `npm run check:task` passes.

The maintainer works trunk-based on `main` and may land your change by rebasing it onto `main` rather than through the merge button. Your authorship is kept.

## Security

Please don't open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).

## Code of Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contribution is licensed under the [MIT License](LICENSE).
