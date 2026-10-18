# Subagent brief

The orchestrator fills the `{…}` fields and sends the text below as the subagent's whole prompt (Agent tool, `model: "sonnet"`). A subagent starts with none of the orchestrator's context, so the ticket body goes in full, never just its number.

---

You are implementing one ticket in the repository checked out at `{WORKTREE_ABSOLUTE_PATH}`. It is a git worktree, detached at the current `main`.

## Ticket #{N}: {TITLE}

{FULL ISSUE BODY: context, acceptance criteria, verification}

## Project context

{1–5 lines: stack, where the relevant code lives, the project's build/test/lint commands, and any CLAUDE.md rules that matter for this ticket.}

## How to work

- Work **only** inside `{WORKTREE_ABSOLUTE_PATH}`. Run every command from there (`cd` into it first) and touch no other directory.
- Implement exactly what the acceptance criteria ask, no more. If you notice another problem, mention it in your report rather than fixing it.
- Use the project's skills for implementation when they fit (test-driven development, incremental implementation). Write or update tests for the behaviour you add.
- Run the project's checks and each verification step from the ticket before finishing.
- Commit your work in this worktree. Use small commits when the work splits naturally, with messages like `<what changed> (#{N})`. Commit everything: leave nothing uncommitted.

## Do not

- Create, switch or push branches, or push anything at all. Do not rebase or merge.
- Create, edit, comment on, close or move any GitHub issue or board item. The orchestrator owns the tracker.
- Guess at a product or design decision the ticket doesn't settle. Stop and report the question instead.
- Touch secret files or anything outside the worktree.

## Report back

End with this report and nothing after it:

```
TICKET: #{N}
RESULT: done | partial | needs-decision
COMMITS: <sha> <message>, one per line
FILES: <paths changed>
CHECKS: <each command you ran> → pass | fail (with the failing output's key lines)
ACCEPTANCE: <each criterion> → met | not met (why)
QUESTIONS: <decisions needed from the user, or "none">
NOTES: <anything the reviewer should look at, or "none">
```
