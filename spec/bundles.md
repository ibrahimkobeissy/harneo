# Spec: bundles

> Module `bundles` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "create all the missing specs, consider that they are validated, create the tasks and implement; I will review everything at the end."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §1 (glossary: bundle), §3 Stage 1.3 (AI dependency detection), Stage 2.3 (AI tag suggestions at import), Stage 3.1 (picking pulls the whole bundle). Visual reference: `design/prototype/flows.js` (Review step: *Proposed bundles*, *Suggested tags*, CLI missing), `library.js` (drawer *Bundle* section, "Bundle of N" badge), `projects.js` (Settings: *Claude CLI* row).
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`library.md`](library.md) (store, preview/import, tags) and [`audit.md`](audit.md) (findings decide what is sent to Claude).

## Objective

Make sure nothing ships half-wired. Harneo records which artifacts **need** which others (a command that calls a skill, a skill that hands off to subagents), so a profile that picks one gets the rest. Links come from three places: the structured references the scan already resolves (certain), a Claude pass that reads the artifacts and proposes links with a reason (the user confirms), and links the user adds by hand. The same Claude pass suggests tags.

**User:** the builder importing a pack they didn't write. Success: importing Addy's agent-skills proposes "`/ship` uses `code-reviewer`, `test-engineer`…" with the reason, the user accepts with one click, and the library card says *Bundle of 4*.

### Decisions (pre-approved 2026-10-23; for the user's review)

1. **A link is directed:** `A needs B`. A's **bundle** is A plus everything reachable from it through confirmed links (transitive). "Bundle of N" counts the members other than A. There is no separate bundle object to name or keep in sync.
2. **Claude runs through the user's `claude` CLI** (`CONTEXT.md` §3 1.3): no API key, no network code in Harneo. Default model **Haiku** (`--model haiku`), changeable in Settings (Haiku, Sonnet, Opus). Detection runs **at import by default** (Settings toggle) and on demand from the library.
3. **What is sent:** each artifact's kind, name, description and main-file text (cut to 6,000 characters), plus a catalogue of the library's artifacts (kind, name, description cut to 120 characters, at most 500 entries) so Claude can link to what is already there. Secrets are already `${KEY}` placeholders in library copies; in the Review step the same replacement is applied before sending. **An artifact with a `plaintext-secrets` or `credential-theft` finding is sent by kind, name and description only.**
4. **Structured references become links automatically** (origin `reference`): a resolved `Reference` whose target is in the library (same kind and name) or in the same import.
5. **Rejected proposals are remembered**, so a re-run doesn't propose them again.
6. **Plugins are not linked inside:** a plugin is kept whole (`CONTEXT.md` §2); it can still be the target or source of a link.

### Scope

**Core (`harneo-core/src/bundles/`)**
- **Schema migration v3:** `settings(key TEXT PRIMARY KEY, value TEXT JSON)` (the app settings store every later module uses: `settings_get(key)`, `settings_set(key, value)`, keys namespaced by module) and `links(from_id, to_id, origin CHECK reference|ai|manual, reason TEXT, status CHECK confirmed|rejected, created_at)` with `PRIMARY KEY(from_id, to_id)`, both ids `REFERENCES artifacts ON DELETE CASCADE`; no self-links.
- **Links API on `Library`:** `links_of(id) -> Links { needs: Vec<LinkView>, needed_by: Vec<LinkView> }`, `bundle_of(id) -> Vec<ArtifactId>` (transitive, cycle-safe, ordered by depth then name), `add_link(from, to, origin, reason)`, `remove_link(from, to)`, `reject_link(from, to)`. `LibraryItem` gains `bundle_size: u32`; `ArtifactDetail` gains `links`.
- **Reference links:** `import` adds origin-`reference` links for the imported artifacts (both directions: new → existing and existing → new when an existing artifact's unresolved reference now resolves).
- **Claude runner (`bundles/claude_cli.rs`), the second process spawn in core after `git.rs`:**
  - finds `claude` on `PATH`; `cli_status() -> CliStatus { found, path, version }` (`claude --version`);
  - runs `claude -p --output-format json --json-schema <schema> --model <m> --no-session-persistence --strict-mcp-config` with **no tools** (verify the exact flags against `claude --help` when building; tools must be disabled so the pass can only read what it is given), the prompt on stdin, the working directory an empty temporary folder (so no project `CLAUDE.md` or settings load), a timeout (180 s per call), and a 1 MiB output cap;
  - arguments only, never a shell; the artifact text is data inside the prompt, and the prompt tells Claude to treat it as data;
  - batches of at most 20 artifacts per call; batches run one after another.
- **Detection:** `detect(inputs, catalogue, model) -> Detection { links: Vec<ProposedLink{from, to, reason, confidence}>, tags: Vec<TagSuggestion{artifact, tags}> }`.
  - The output is validated against the schema; ids not in the batch or catalogue, self-links, confirmed or rejected pairs and duplicates are dropped; reasons are cut to 200 characters; tags must match the tag name rules (lowercase, digits, dashes, ≤ 40 chars), at most 3 per artifact, existing tag names preferred.
  - Errors: `ClaudeMissing`, `ClaudeTimeout`, `ClaudeFailed{message}` (stderr cut to 500 chars, never the prompt), `ClaudeBadOutput`.
- **Preview integration:** `detect_for_preview(preview_id, selection) -> Detection` over the ticked candidates (prepared with the same secret replacement as the copy). `import` gains `links: &[(ArtifactId, ArtifactId, String)]` for the accepted AI links.
- **Library integration:** `detect_in_library(ids | all) -> Detection` over stored copies; `accept_links` stores the chosen ones as `ai`, confirmed.

**App**
- **IPC:** `claude_cli_status`, `bundles_detect_preview` (async), `bundles_detect_library` (async), `links_of`, `link_add`, `link_remove`, `link_reject`, `bundle_of`; `library_import` takes the accepted links. `settings_get` / `settings_set`; this module's keys are `bundles.model` and `bundles.atImport`.
- **Import Review step** (prototype `flows.js:38-50`): while Claude reads, "Claude is reading N artifacts…" with progress per batch; *Proposed bundles* (one group per lead with "+ N", the reason, member chips with ×, an accept toggle per group, "No bundles found: these artifacts don't depend on each other."); *Suggested tags* merges frontmatter keywords and Claude's tags, removable, **plus an add-tag input** (closes the prototype gap); the CLI-missing callout and the error callout ("Claude couldn't finish: <reason>. Import without bundles, or retry.") with **Retry**. Detection off in Settings → the sections say so with a "Detect now" button.
- **Library:** "Bundle of N" badge on cards and a Bundle column in the list; the detail panel's **Bundle** section ("installed together, always"): *Needs* and *Needed by* lists with origin chips (`reference`, `Claude`, `manual`), each opening that artifact with a Back trail; **Add a link…** (artifact picker), remove (×) on `ai` and `manual` links (reference links are explained, not removable).
- **Find bundles with Claude** (library toolbar overflow and the detail panel): runs `bundles_detect_library` on the current selection or the whole library and shows the proposals in a dialog with the same component as the Review step.
- **Settings screen:** *Claude CLI* row (Found + path + version / Not found with the prototype copy), model select, "Detect bundles and tags at import" toggle.

**Out of scope:** profiles pulling bundles (`profiles`); a judgement of whether a profile is good (`CONTEXT.md` §6); linking inside plugins; sending anything anywhere but the local `claude` CLI.

## Tech Stack

No new dependency. The `claude` CLI is a **runtime** prerequisite for detection only, checked at run time; `BUILDING.md` lists it as optional.

## Commands

```bash
cargo test -p harneo-core bundles
npx vitest run src/features/import src/features/library
```

Tests never run the real `claude`: the runner takes the program path from a `ClaudeConfig` (like `GitConfig`), and tests point it at a fixture script that prints canned JSON, sleeps (timeout), exits non-zero, or prints garbage.

## Project Structure

```
crates/harneo-core/src/bundles/
  mod.rs          → Link types, links API, bundle_of, reference links
  claude_cli.rs   → CliStatus, the runner (spawn, timeout, output cap)
  detect.rs       → prompt + schema, batching, output validation
  *_tests.rs
src/features/import/BundleReview.tsx, src/features/library/BundleSection.tsx, FindBundlesDialog.tsx
```

## Code Style

```rust
/// Runs one detection batch through the user's `claude` CLI. The prompt goes on stdin,
/// never on the command line, and Claude gets no tools: it can only read what we send.
pub(crate) fn run_batch(cfg: &ClaudeConfig, prompt: &str, schema: &str) -> Result<Detection> {
    let out = cfg.command()                       // Command::new(&cfg.program), args only
        .args(["-p", "--output-format", "json", "--json-schema", schema, "--model", &cfg.model])
        .current_dir(cfg.scratch.path())
        .run_with_stdin(prompt, cfg.timeout, OUTPUT_LIMIT)?;
    validate(parse(&out)?)
}
```

## Testing Strategy

- Links: add/remove/reject, transitive `bundle_of` with a cycle, cascade on artifact removal, reference links created at import in both directions.
- Runner with fixture scripts: success, timeout, non-zero exit, bad JSON, oversized output, missing binary; the prompt is on stdin and an artifact with a secret finding is sent without its text (the fixture script writes its stdin to a file the test inspects); no secret value ever appears in the prompt.
- Validation: unknown ids, self-links, rejected pairs and bad tag names are dropped.
- Frontend (mockIPC): Review step proposals (accept, drop member, retry, CLI missing, detection off), detail Bundle section (add, remove, navigate), Find bundles dialog, Settings row.

## Boundaries

- **Always:** prompt on stdin, no tools for Claude, temp working dir, timeout; placeholders before sending; user confirms every AI link.
- **Ask first:** sending more than the decided fields; a new process other than `git` and `claude`.
- **Never:** store or send a secret value; run anything an artifact contains; add an AI link without the user's confirmation.

## Success Criteria

1. Importing a pack whose command names a skill records a `reference` link without Claude.
2. With the CLI present, the Review step shows Claude's proposed links with reasons and tag suggestions; accepted ones are stored, rejected ones are not proposed again.
3. Without the CLI, import works and the Review step says why bundles are missing.
4. The library shows *Bundle of N*, the detail panel lists needs / needed-by, and links can be added and removed by hand.
5. No secret value or secret-flagged artifact text reaches the `claude` process (test).
6. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: Haiku as the default model; detection on by default at import; the 6,000-character cut.
