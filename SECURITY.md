# Security policy

Harneo copies third-party AI artifacts (skills, hooks, MCP servers and more) into your projects, so its own security matters.

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through GitHub: [Security → Report a vulnerability](https://github.com/ibrahimkobeissy/harneo/security/advisories/new).

Include what you found, how to reproduce it, and the impact you expect. You should get a first answer within 7 days. Harneo is maintained by one person in their spare time, so a fix can take longer; you will be kept informed and credited in the advisory unless you prefer not to be.

## Scope

In scope:

- Writes outside the target project or Harneo's data folder, or loss of a project file without a backup.
- Secrets read, stored or sent where Harneo says they are not (MCP secrets are meant to exist only as `${VAR}` placeholders).
- Data sent to Claude or Sentry beyond what [the README](README.md#privacy) and Settings describe.
- Ways for an imported artifact to bypass or fool the security audit, including prompt injection against the Claude confirmation step.
- Code execution from an imported artifact or a git URL during import, audit or scan.

Out of scope: the content of third-party artifacts themselves (report those to their authors), and vulnerabilities in Claude Code, Tauri or other dependencies (report those upstream; tell us if Harneo should react).

## Supported versions

Harneo is pre-1.0 and distributed as source. Only the latest `main` is supported; fixes land there.
