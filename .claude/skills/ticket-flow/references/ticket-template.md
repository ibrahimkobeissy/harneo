# Ticket template

Title: imperative and specific, e.g. `Import artifacts from a local folder`, not `Import`.

Body:

```markdown
## Context
Why this change is needed, and where it fits (link the spec section or the parent ticket).

## Scope
What to build. Also say what is explicitly out of scope, when that could be unclear.

## Acceptance criteria
- [ ] Observable behaviour 1
- [ ] Observable behaviour 2

## Verification
Commands to run and what passing looks like, e.g. `cargo test import::` passes; `npm run build` succeeds.

## Dependencies
Blocked by #12
Blocked by #14
```

Rules:
- Sized for one subagent in one sitting. If the criteria list grows beyond about 5 items, or the ticket spans unrelated areas, split it and link the parts with `Blocked by`.
- Write one `Blocked by #N` per line. Leave the Dependencies section out when there are none.
- If the ticket depends on a choice the user hasn't made, add the `needs-decision` label and state the question under a `## Decision needed` heading. Triage keeps it in Backlog until the question is answered and the label removed.
