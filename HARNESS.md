# Harness

Installed by Harneo from profile **Software Factory v5** on 2026-11-01.

| Artifact | Type | Source | Why it's here |
|---|---|---|---|
| session-radar | Plugin | ibrahimkobeissy/claude-mods | A live sidebar for long sessions: usage limits, context, agents, background shells and progress at a glance |
| waiting-bell | Plugin | ibrahimkobeissy/claude-mods | Rings a bell and shows a banner whenever Claude is blocked waiting for you, repeating every 5 minutes until you answer |
| agent-skills | Plugin | addyosmani/agent-skills | Production-grade engineering skills for AI coding agents — covering the full software development lifecycle from spec to ship |
| impeccable | Plugin | pbakaus/impeccable | Design fluency for frontend development |
| security-guardrails | Skill | Local | Explains and maintains this project's security guardrails: the deny rules and the guard.py hook that stop agents from reading credentials and secrets, leaking tokens through gh, or destroying git work |
| security-guardrails | Rule | Local |  |
| settings | Settings | Local |  |

## Plugins

Plugins are not copied: they install from their own marketplace. `.claude/settings.json` enables them, and Harneo installs them for this project through the `claude` CLI when it scaffolds or refits it. Anyone else who opens the project is offered them by Claude Code when trusting the folder.

## Files

- `.claude/hooks/guard.py`
- `.claude/rules/security-guardrails.md`
- `.claude/settings.json`
- `.claude/skills/security-guardrails/SKILL.md`
- `.claude/skills/security-guardrails/scripts/install-gitleaks-hook.sh`
- `.claude/skills/security-guardrails/scripts/selftest.py`
- `HARNESS.md`

Managed by Harneo: edit the library copy, or your edits show as drift.
