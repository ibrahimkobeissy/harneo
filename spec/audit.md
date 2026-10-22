# Spec: audit

> Module `audit` of the [capability map](capabilities.md). Status: **pre-approved by the user (2026-10-23): "consider it as validated, create the tasks and implement; I will review everything after the implementation."**
> Product context: [`CONTEXT.md`](../CONTEXT.md) §3 Stage 1.4 (security audit on arrival, permission transparency), §4 (inspired by HarnessKit's audit rules), §7 (what to borrow from HarnessKit and its flaws). Visual reference: `design/prototype/security.js`, the trust meter and filters in `library.js`, the Review step in `flows.js`.
> Conventions: [`app-shell.md`](app-shell.md). Builds on [`artifact-model.md`](artifact-model.md) (scanned artifacts, safe walk, `literal_secret`) and [`library.md`](library.md) (store, import, queries).

## Objective

Tell the user, for every artifact, how far to trust it and exactly what it can reach, before it enters the library and at any time after. Each artifact gets a **trust score** (0–100) with a **tier**, a list of **findings** (rule, severity, file and line, short evidence) and its **reach** (files, network hosts, shell commands, environment variables). Everything is static: Harneo reads the artifact's files and never runs them.

**User:** the builder importing packs from strangers (`CONTEXT.md` §1). Success: a dangerous artifact is visibly flagged and starts excluded at import; a clean one shows *Trusted* with its reach spelled out.

### Decisions (user, 2026-10-23)

1. **Rules are ported from HarnessKit** (Apache-2.0, commit `9098728d22fd5c2ea6fb4864da19183b55cb33c4`), with attribution, and extended to Harneo's 10 artifact types and the gaps the research found.
2. **Trust score 0–100 with 4 tiers.**
3. **At import, an artifact with a critical finding starts in Excluded**, with the finding shown; the user can restore it. Nothing is silently blocked.
4. **Reach is static**, read from the files. No AI review in this module.

### Scope

**Core (`harneo-core/src/audit/`)**
- **Engine.** An `AuditRule` trait (`id`, `severity`, `check(&AuditInput) -> Vec<Finding>`); `audit(&Artifact, files) -> Audit { score, tier, findings, reach }`. The input is every file of the artifact read through the safe walk (size and read limits apply; secret-pattern and symlinked files are never read), not only the main file: a skill's scripts, a plugin's whole folder (recursive, code and JSON), Settings and its hook scripts, an MCP server's definition.
- **Finding:** `rule` (stable id), `severity` (`critical`, `high`, `medium`, `low`), `message`, `file`, `line`, `evidence` (a short excerpt; a secret value is always replaced by `<redacted>` and never stored or sent over IPC).
- **Score** (HarnessKit's formula): start at 100; the first finding of each rule deducts by severity (critical 25, high 15, medium 8, low 3); each further finding of the same rule deducts 1; floor 0.
- **Tiers:** **Trusted** 80–100, **Review** 60–79, **Risky** 40–59, **Dangerous** 0–39. Colours from `DESIGN.md` (green, amber, orange, red state washes).
- **Rules** (ids are stable; each has a triggering and a non-triggering test). Fenced code blocks **are** scanned (unlike HarnessKit, where a command in a ```` ```bash ```` block — exactly what an agent copies — is skipped); prose that only *mentions* a pattern inside inline backticks is treated as documentation for the safety-bypass flags, as HarnessKit does.
  - `prompt-injection` (critical): instruction-override phrases (HarnessKit's set, extended), fake system blocks, instructions to hide behaviour from the user. Applies to every Markdown kind and prompt-type hooks.
  - `hidden-unicode` (high): zero-width, bidi and tag characters (the HarnessKit list), reported with their position instead of silently removed.
  - `rce` (critical): download piped to a shell or interpreter (`curl|wget … | sh|bash|zsh|python`), `base64 -d |`, `eval` of fetched content, download then `chmod +x` and run.
  - `credential-theft` (critical when a read of a credential path and a network send are within 20 lines, else high): HarnessKit's targets plus `~/.config/gh`, `~/.npmrc`, `~/.docker/config.json`, `~/.kube`, `.git-credentials`, `~/.claude/.credentials.json`, keychain tools, `printenv`/`env` dumps.
  - `plaintext-secrets` (critical): the shared `literal_secret` detector extended (GitHub `ghp_`/`gho_`/`ghs_`/`ghu_`/`github_pat_`, Slack, Anthropic `sk-ant-`, OpenAI `sk-`, AWS `AKIA`, Google `AIza`, Stripe `sk_live_`, PEM private-key blocks, JWTs, `user:pass@` in URLs), matched inside `KEY=…`, quotes and `Bearer …`.
  - `safety-bypass` (critical): `--dangerously-skip-permissions`, `defaultMode: bypassPermissions`, `allowedTools: "*"` / `tools: "*"`, `--no-verify`, instructions to skip confirmation; Settings `permissions.allow` granting `Bash(*)` or `Bash` unrestricted.
  - `dangerous-commands` (high in hooks and Settings, medium elsewhere): HarnessKit's list plus `rm -rf ~`/`$HOME`, `find / -delete`, `git push --force` / `reset --hard`, shell rc-file edits, `crontab`, `git config --global`.
  - `broad-permissions` (high): an MCP server binding `0.0.0.0`/`*`, a filesystem server on `/` or `~`, Settings allowing every tool, a subagent granted every tool.
  - `supply-chain` (medium): `npx`/`uvx`/`bunx`/`pipx` packages without a pinned version, `docker run` with `:latest` or no tag, remote MCP over plain `http://`, `enableAllProjectMcpServers: true`, a plugin's `package.json` lifecycle scripts (high when they download or run a shell).
  - `mcp-command-injection` (high): `$(` or backticks in an MCP server's arguments.
  - `permission-combo` (high): reach that combines network with env access (exfiltration) or network with shell (remote code), pointing at the lines that produced it.
- **Reach**, inferred from the same files (HarnessKit's five dimensions minus databases): `files` (sensitive or absolute paths, read-only vs write when clear), `network` (hosts from URLs; `*` for package runners), `shell` (commands from shell code blocks, hook commands, MCP commands, Settings `allow` entries), `env` (MCP env and header keys, Settings `env` keys — names only, never values). Remote MCP URLs drop `user:pass@`.
- **Attribution:** ported code carries a header naming HarnessKit, its commit and Apache-2.0, and saying it was modified; the Apache-2.0 text is added as `licenses/HarnessKit-APACHE-2.0.txt` and a `THIRD_PARTY_NOTICES.md` at the root lists it (HarnessKit ships no NOTICE file).

**Library integration (`harneo-core/src/library/`)**
- **Preview** audits each candidate; `Candidate` gains `audit`. **Import** stores the audit (schema migration v2: `trust_score`, `tier`, `findings` JSON, `reach` JSON, `audited_at`).
- **Re-audit:** `Library::reaudit_all()` re-runs the audit on every stored artifact's library copy (after a rule change), reporting how many changed.
- **Queries:** filter by minimum trust and by tier, a *Has findings* filter, sort by trust, tier facet counts; `LibraryItem` gains `trustScore`, `tier`, `findingCount`.

**CLI:** `harneo audit <path> [--json]` scans and audits a folder, printing each artifact's score, tier, findings and reach; exit 0 when the audit ran.

**App (`src/features/security/`, `src/features/library/`, `src/features/import/`)**
- **Library:** trust meter on cards (footer) and a Trust column in the list (sortable); filter column gains *Trust ≥* (slider) and *Has findings*; sort by trust; toolbar chips for them.
- **Detail panel:** a Trust section: score, tier, findings grouped by severity (rule, message, file:line, evidence), and Reach (four rows).
- **Import:** each Choose row shows its tier; a candidate with a critical finding starts in **Excluded** with "Critical finding: <rule>" and can be restored; the Review step gets a *Security audit* summary (tiers count, findings by severity, the artifacts that reach network or shell).
- **Security page** (prototype `security.js`): trust-score distribution (histogram coloured by tier), findings by severity, reach tiles (network hosts, shell, env vars, any file) that filter the reach table, a sortable findings table and a sortable reach table, and **Re-run audit**.

**Out of scope:** AI judgement of intent (later, with `bundles`'s Claude pass); suppressing or accepting a finding (noted for later: a per-artifact "accept risk" with a reason); vulnerability databases for packages; auditing artifacts already installed in projects (`drift`).

## Tech Stack

New dependency (pre-approved with this spec): `regex` (patterns compiled once with `LazyLock`; the `regex` crate has linear-time matching, so a crafted file can't cause catastrophic backtracking). Everything else exists already.

## Commands

The project commands, plus:

```bash
cargo test -p harneo-core audit          # rules, score, reach
cargo run -p harneo-cli -- audit <path>  # audit a folder
npx vitest run src/features/security
```

## Project Structure

```
crates/harneo-core/src/audit/
  mod.rs        → Audit, Finding, Severity, Tier, Reach, audit(), score
  input.rs      → files of an artifact read through the walk; line index
  rules/        → one file per rule family: injection.rs, secrets.rs, commands.rs, mcp.rs, supply_chain.rs, combo.rs
  reach.rs      → reach inference
crates/harneo-core/tests/audit_*.rs → fixture tests (hostile packs built at test time)
licenses/HarnessKit-APACHE-2.0.txt, THIRD_PARTY_NOTICES.md
src/features/security/ → Security page
```

## Code Style

```rust
/// Ported from HarnessKit (crates/hk-core/src/auditor/rules/content.rs, commit 9098728, Apache-2.0),
/// modified: fenced code is scanned, all Markdown kinds are inputs, evidence is redacted.
pub(crate) struct Rce;
impl AuditRule for Rce {
    fn id(&self) -> &'static str { "rce" }
    fn severity(&self) -> Severity { Severity::Critical }
    fn check(&self, input: &AuditInput) -> Vec<Finding> { … }
}
```

Rules are pure over `AuditInput`; only `input.rs` reads files (through the walk). A rule never panics; a regex that fails to compile is a test failure, never a runtime error.

## Testing Strategy

- Every rule: a triggering and a non-triggering test, plus the false-positive case the research found (e.g. `credential-theft` with read and send far apart stays high, not critical).
- Score and tiers: table tests at the boundaries (79/80, 59/60, 39/40) and the repeat-finding −1.
- Secrets: a test reads every serialized `Audit` and every byte of the store after import and never finds a fixture secret value.
- Hostile fixture packs built at test time (no secret-shaped strings committed; gitleaks stays clean).
- Library: migration v1→v2 keeps existing rows (audited on next re-audit); preview carries the audit; critical candidates are flagged; queries filter and sort by trust.
- Frontend: Security page, trust meter, filters, detail Trust section, import exclusion of critical candidates — with `mockIPC`.
- Performance: auditing a 200-artifact pack takes under 2 s.

## Boundaries

- **Always:** never execute or evaluate artifact content; redact secrets in evidence; attribute ported code; keep rule ids stable.
- **Ask first:** changing the score formula or tier boundaries after release (stored scores would drift).
- **Never:** block an import on a finding; send artifact content anywhere; store a secret value.

## Success Criteria

1. Every rule in Scope has an id, a severity and both tests; the hostile fixtures trigger each one.
2. Score and tiers follow the formula and boundaries exactly.
3. At import, critical candidates start excluded with the reason shown; restoring one imports it with its findings.
4. The library shows trust on cards and in the list, filters and sorts by it; the detail panel shows findings and reach; the Security page matches the prototype in light and dark.
5. No secret value appears in any finding, stored row or IPC payload.
6. `harneo audit <path>` prints the same results as the app.
7. Ported code is attributed and the Apache-2.0 text is included.
8. `npm run check:full` passes and the coverage ratchet holds.

## Open Questions

None blocking. For the user's review: the four tier names and boundaries, and whether "accept risk" belongs in v1.
