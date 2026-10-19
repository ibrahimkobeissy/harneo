---
name: ticket-flow
description: >-
  Run a project's work from GitHub Issues and a GitHub Projects board, trunk-based
  on main. Every change gets a ticket. An orchestrator session picks up to 3
  Ready tickets, hands each to a Sonnet subagent in its own worktree off main,
  then validates, fixes, merges and pushes the work itself. The board Status
  (Backlog → Ready → In progress → In review → Done) is updated at each step.
  Use when the user asks what's on the backlog, to pick, start or run tickets,
  to create, update, triage or close a ticket, to move a ticket on the board, or
  to "run the cycle"; and before making any code change in a project that uses
  this skill.
---

# Ticket Flow

GitHub Issues hold the backlog. A GitHub Projects (v2) board shows where each ticket is. Work lands straight on `main`: no feature branches and no PRs. One orchestrator session (usually Opus) owns the tracker, the git history and the quality bar. Sonnet subagents only write code.

## Rules that always apply

1. **No change without a ticket.** Before editing code, find the issue for the change or create one (see "Writing tickets"). This applies to small asks made in chat too. A fix that the orchestrator makes while validating a ticket belongs to that ticket. An unrelated problem gets its own new ticket.
2. **Only the orchestrator writes to the tracker.** Subagents never create, edit, comment on, close or move tickets. With one writer, statuses can't race and nothing is moved twice.
3. **Never delete a ticket.** To drop one, close it with a reason: `gh issue close N --reason "not planned"`, or close it as a duplicate with a comment naming the original.
4. **Trunk-based.** Nothing is ever pushed except `main`. Worktrees are detached checkouts of `main` (see "Worktrees on main"). No branch is created, locally or on the remote.
5. **The orchestrator never hands work back.** If a subagent's work fails validation, the orchestrator fixes it, tests it and merges it. It does not resume, message or re-brief that subagent.

## Board statuses

| Status | Meaning | Set by the orchestrator when |
| :--- | :--- | :--- |
| **Backlog** | Not workable yet: blocked by an open ticket, waiting on a decision (`needs-decision` label), or not triaged | the ticket is created and isn't Ready |
| **Ready** | Not blocked, no decision pending: can be picked | triage finds it unblocked with no `needs-decision` label |
| **In progress** | Picked; a subagent (or the orchestrator itself) is working on it | right before the subagent is launched |
| **In review** | The work is finished and waiting for the orchestrator to validate and merge it | the subagent returns (the orchestrator moves it at once) |
| **Done** | Merged to `main` and pushed; the issue is closed | after the push |

A ticket that turns out to need a decision while in progress goes back to **Backlog**, gets the `needs-decision` label and a comment stating the question, and the user is told.

Move statuses with `scripts/board.sh` (paths in this skill are relative to the skill folder):

```bash
S=.claude/skills/ticket-flow/scripts/board.sh
$S list              # every ticket on the board with its status
$S list ready        # only Ready tickets
$S add 12 ready      # put issue #12 on the board in Ready
$S set 12 progress   # backlog | ready | progress | review | done
$S check             # verify config, auth and the board's Status options
$S setup             # create missing Status options; keeps every card's status
```

## The cycle (orchestrator)

Run this when the user asks what's on the backlog, says to take tickets, or says to run the cycle. Keep cycling until nothing is Ready or the user says stop. After each cycle, give a short summary: the tickets merged, any new tickets, and anything waiting on the user.

### 1. Sync and triage

```bash
git checkout main && git pull --ff-only origin main
```

Then triage every open ticket that isn't In progress or In review:
- Read its `Blocked by #N` lines. It is **blocked** if any of those issues is still open.
- It **needs a decision** if it has the `needs-decision` label.
- Neither applies → set it to **Ready**. Otherwise → **Backlog**.

Run triage again after every merge, because a closed ticket may unblock others.

### 2. Pick up to 3

Choose up to 3 Ready tickets, in the order listed in `tasks/plan.md` if the project has one, otherwise oldest first. **Avoid overlap:** two tickets likely to edit the same files go in different cycles, so take fewer than 3 rather than set up a merge conflict. If nothing is Ready, say so, list what is blocked and which decisions are pending, and stop.

### 3. Start each ticket

For each picked ticket, in this order:

```bash
$S set N progress
git worktree add --detach .claude/worktrees/N main
```

Then launch one subagent per ticket with the Agent tool: `model: "sonnet"`, all of them in **one message** so they run in parallel, in the background. Fill each one's prompt from `references/subagent-brief.md`, including the full ticket body and the absolute worktree path. Don't use the Agent tool's own `isolation: "worktree"` option, because it creates a branch. This skill creates the worktree itself.

### 4. When a subagent returns

Move the ticket at once: `$S set N review`. Then validate that ticket's worktree yourself. Don't trust the subagent's report on its own.

1. **Rebase onto the current main** (another ticket may have merged first): `git -C .claude/worktrees/N rebase main`. Resolve any conflicts yourself.
2. **Review the diff** against the ticket's acceptance criteria: `git -C .claude/worktrees/N diff main`. Look for missing criteria, unrelated changes, and anything the subagent left unfinished or uncommitted.
3. **Run the project's checks** in the worktree: build, lint, typecheck and tests, plus each verification step in the ticket.
4. **Fix anything wrong yourself** in that worktree, commit it with `(#N)` in the message, and re-run the checks. Don't report back to the subagent.

If the ticket can't be finished without the user, move it to Backlog with `needs-decision`, comment the question on the issue, remove the worktree, and carry on with the other tickets.

### 5. Merge (one ticket at a time)

```bash
sha=$(git -C .claude/worktrees/N rev-parse HEAD)
git merge --ff-only "$sha"        # in the main checkout
git push origin main
gh issue close N --reason completed --comment "Merged to main in $sha."
$S set N done
git worktree remove .claude/worktrees/N
```

If `--ff-only` fails, main moved after your rebase: rebase again (step 4.1), re-run the checks, and retry. Merge the next returned ticket only after this push. Then re-triage, and start the next cycle once every ticket in this one is Done or back in Backlog.

### Small changes without subagents

For a quick change that the orchestrator makes itself: create or find the ticket, set it to In progress, work in the main checkout, run the checks, commit with `(#N)`, push, close the issue, and set it to Done. Skip In review, since the session that wrote the code is the one validating it.

## Worktrees on main

Git won't let two worktrees check out the same branch, so each ticket's worktree is a **detached** checkout of `main`'s current commit. The subagent commits there on no branch at all. The orchestrator replays those commits onto the latest `main` (`rebase main`) and fast-forwards `main` to them. History stays linear, everything lands on `main`, and no branch ever exists. Worktrees live in `.claude/worktrees/`, which must be in `.gitignore`.

## Writing tickets

Use `references/ticket-template.md` for every new ticket. One ticket = one change a subagent can finish and verify on its own. After creating it: `$S add N backlog` (or `ready` if it already passes triage). Write dependencies as `Blocked by #N` lines, one per line, because triage parses them. Use sub-issues for an epic's children if the project wants that grouping.

Creating a ticket while planning (e.g. with Addy Osmani's `planning-and-task-breakdown`) follows the same template. `tasks/plan.md` then lists only the issue links in order, never a duplicate checklist. Don't use `tasks/todo.md` or Addy's `/build auto` in a project that runs this cycle: the cycle replaces them. Their implementation skills (TDD, incremental implementation, code review) still apply inside each ticket.

## Environments

- **Local session with `gh`** (the normal case): everything above works. The board needs the `project` token scope, which you add with `gh auth refresh -s project`.
- **Cloud session** (no working `gh`, or no Projects access): read and write issues through the GitHub MCP tools instead of `gh issue`. The board can't be reached there, so record the status as a label (`status:backlog`, `status:ready`, `status:progress`, `status:review`, `status:done`) and say so. The next local session runs `$S sync`, which moves each labelled ticket to that status on the board and removes the label.

## Setup in a new project

1. Copy this folder to `.claude/skills/ticket-flow/`. The script needs `gh` and `jq`.
2. Create `.claude/ticket-flow.conf`:
   ```
   OWNER=<github user or org that owns the project board>
   REPO=<owner>/<repo>
   PROJECT=<project number>
   ```
3. Run `$S setup` to give the board's **Status** field the options Backlog, Ready, In progress, In review, Done. It keeps every card's current status and removes default options (such as `Todo`) that no card uses.
4. In the project's settings → Workflows, turn on **Auto-add to project** for the repo, with new items set to Backlog. That way no issue is missed.
5. Create the `needs-decision` label: `gh label create needs-decision -R <owner>/<repo> --color D93F0B`.
6. Add `.claude/worktrees/` to `.gitignore`.
7. Run `$S check`.
8. Point the project's `CLAUDE.md` at this skill ("Tasks: follow the ticket-flow skill").
