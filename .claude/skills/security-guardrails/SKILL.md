---
name: security-guardrails
description: >-
  Explains and maintains this project's security guardrails: the deny rules
  and the guard.py hook that stop agents from reading credentials and secrets,
  leaking tokens through gh, or destroying git work. Use when a command was
  blocked or asked for approval by security-guardrails, when the user asks why
  something is blocked or wants an exception, wants to review guard.log, run
  the self-test, or install the gitleaks pre-commit hook.
---

# Security Guardrails

Three layers stop an agent from reading secrets, leaking them, or destroying
work. Each covers a gap the others leave.

| Layer | Where | What it does |
| --- | --- | --- |
| **Deny rules** | `permissions.deny` in `.claude/settings.json` | Block Claude's own Read/Edit/Write tools on secret paths |
| **Guard hook** | `.claude/hooks/guard.py` (PreToolUse) | Checks every tool call: secret paths, token leaks (`gh auth token`, `printenv`), `gh` misuse, destructive git, edits that weaken the guardrails, and the first commit without gitleaks. Answers deny or ask |
| **Rules** | `.claude/rules/security-guardrails.md` | The same policy in prose, for agents that ignore Claude settings |

**Limits, said plainly.** There is no OS-level sandbox. The guard reads
command text, so a determined agent can disguise a shell command (an
interpreter that builds a path at runtime, an encoded string) and nothing at
the OS level stops it: shell commands can read anything your user can, write
anywhere, and reach any host. The deny rules cover only Claude's own file
tools. Plugin hooks and MCP servers are not checked at all (review them before
installing). Shell commands still go through the normal permission prompts.

## When something is blocked

- **"Blocked by security-guardrails: ..."** means guard.py denied it. Don't try
  another way to do the same thing. If the user really needs it, they do it
  themselves.
- **"security-guardrails: ..." with an approval prompt** means guard.py asked:
  the user decides.

## Exceptions (always with the user's approval)

- `.env.example` and similar templates are blocked like `.env`. The user can
  create or show them.

guard.py asks before any edit that weakens `settings.json` (removing a deny rule
or the hook, `disableAllHooks`, re-enabling bypass mode).

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
