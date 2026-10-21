# Harneo

Harneo is a harness manager for Claude Code: a free, open-source desktop app, built with Tauri, that keeps a library of AI Artifacts (skills, commands, subagents, settings, MCP servers and more), composes profiles from it, and scaffolds or refits projects from a profile. It is a personal tool, shared openly. Distribution is source only: you build it for your own OS.

To build it, start with [BUILDING.md](BUILDING.md).

## Commands

| Goal                        | Command                                                |
| --------------------------- | ------------------------------------------------------ |
| Run the app (dev)           | `npm run tauri dev`                                    |
| Build the app (release)     | `npm run tauri build -- --no-bundle`                   |
| Run the CLI                 | `cargo run -p harneo-cli -- --version`                 |
| Test                        | `npm test` and `cargo test --workspace`                |
| Lint                        | `npm run lint`                                         |
| Fix lint and format         | `npm run fix`                                          |
| Gate after each edit        | `npm run check:fast`                                   |
| Gate before a ticket is done | `npm run check:task`                                  |
| Gate before merge to `main` | `npm run check:full`                                   |

`CONSTRAINTS.md` defines what each check runs.

## License

MIT, see [LICENSE](LICENSE).
