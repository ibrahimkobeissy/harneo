---
name: security-guardrails
description: >-
  Explains and maintains this project's security guardrails: the OS-level
  sandbox, the deny rules and the guard.py hook that stop agents from reading
  credentials and secrets, leaking tokens through gh, or destroying git work.
  Use when a command was blocked or asked for approval by security-guardrails,
  when a tool fails because of the sandbox, when the user asks why something is
  blocked or wants an exception (a command outside the sandbox, a new network
  domain), wants to review guard.log, run the self-test, or install the
  gitleaks pre-commit hook.
---

# Security Guardrails

Four layers stop an agent from reading secrets, leaking them, or destroying
work. Each covers a gap the others leave.

| Layer | Where | What it does |
| --- | --- | --- |
| **Sandbox** | `sandbox` in `.claude/settings.json` | Every shell command runs in an OS-level box (bubblewrap on Linux): secret paths can't be opened by any program, writes stay in the project, network only to allowed domains. Sandboxed commands run without a prompt |
| **Deny rules** | `permissions.deny` in `.claude/settings.json` | Block Claude's own Read/Edit/Write tools on secret paths. Also fed into the sandbox |
| **Guard hook** | `.claude/hooks/guard.py` (PreToolUse) | Checks every tool call: secret paths, token leaks (`gh auth token`, `printenv`), `gh` misuse, destructive git, edits that weaken the guardrails, and the first commit without gitleaks. Answers deny or ask |
| **Rules** | `.claude/rules/security-guardrails.md` | The same policy in prose, for agents that ignore Claude settings |

**Limits, said plainly.** The guard reads command text, so a determined agent
can disguise a command. The sandbox is the layer that can't be talked around,
but it covers shell commands only: Claude's file tools rely on the deny rules,
and plugin hooks and MCP servers run outside it (check them before installing).

## When something is blocked

- **"Blocked by security-guardrails: ..."** means guard.py denied it. Don't try
  another way to do the same thing. If the user really needs it, they do it
  themselves.
- **"security-guardrails: ..." with an approval prompt** means guard.py asked:
  the user decides.
- **A command fails with a permission or network error** but isn't blocked by
  guard.py: that's the sandbox. Commands are not retried outside it
  (`allowUnsandboxedCommands: false`). Tell the user which command failed and
  why, and offer one of the fixes below.

## Exceptions (always with the user's approval)

- **A command that needs to run outside the sandbox** (Docker, a tool that needs
  your keyring): add it to `sandbox.excludedCommands`, e.g. `"docker compose *"`.
  It then goes through the normal permission flow and guard.py.
- **A network domain sandboxed commands need**: add it to
  `sandbox.network.allowedDomains`. This one is not treated as weakening.
- **A command in `sandbox.excludedCommands` (`gh`, `git push`, `git pull`…)
  is exempt only when it is the whole Bash call.** Chained (`cd x && gh …`,
  `git pull | tail`, `$(…)`) or prefixed (`VAR= gh`, `timeout 30 git push`), it
  runs sandboxed without `~/.config/gh` or `~/.ssh`, and fails as "not logged
  in" or "check access rights", which looks like broken auth but isn't.
  The spelling must match the entry too: `git -C dir push` is not `git push *`,
  and `/abs/path/.claude/skills/…/board.sh` is not `.claude/skills/…/board.sh *`.
  guard.py refuses all of these up front: run the command as its own call,
  spelled as the entry is, `cd` in a separate call first if needed, and use
  `-R owner/repo` or `--body-file` instead of substitutions. A script that calls
  `gh` inside (e.g. ticket-flow's `board.sh`) needs its own `excludedCommands`
  entry.
- `.env.example` and similar templates are blocked like `.env`. The user can
  create or show them.

guard.py asks before any edit that weakens `settings.json` (removing a deny rule
or the hook, turning the sandbox off, excluding more commands, opening paths).

## guard.log

Every deny and ask is one JSON line in `.claude/hooks/guard.log` (kept out of git
by `.claude/hooks/.gitignore`, which the hook creates),
with the time, decision, tool, reason and the first 200 characters of the
command. Read it to spot false positives:

```bash
tail -n 20 .claude/hooks/guard.log
```

## Self-test

Run after any change to guard.py or settings.json:

```bash
python3 .claude/skills/security-guardrails/scripts/selftest.py
```

It feeds real events through the hook (deny, ask and allowed cases), tests the
settings-weakening check and the gitleaks flow in temp folders, and checks that
a crash blocks.

## gitleaks

gitleaks scans what is about to be committed and refuses the commit if it finds
a secret. The first `git commit` in a repo without the hook is denied by
guard.py with the command to run:

```bash
bash .claude/skills/security-guardrails/scripts/install-gitleaks-hook.sh
```

Run it, then commit again. It keeps any existing pre-commit hook (and puts the
scan in `pre-commit-user` when GitButler manages the hook). If gitleaks isn't
installed on the machine, commits are not blocked; the hook only warns.
