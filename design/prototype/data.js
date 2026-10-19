/* Harneo prototype: synthetic demo data.
   Names from addyosmani/agent-skills are real; everything else is invented for the prototype. */
(function () {
  "use strict";

  const TYPES = {
    skill: { label: "Skill", plural: "Skills", icon: "sparkles", tint: "blue" },
    subagent: { label: "Subagent", plural: "Subagents", icon: "bot", tint: "violet" },
    command: { label: "Command", plural: "Commands", icon: "terminal", tint: "cyan" },
    rule: { label: "Rule", plural: "Rules", icon: "scale", tint: "slate" },
    mcp: { label: "MCP server", plural: "MCP servers", icon: "plug", tint: "orange" },
    plugin: { label: "Plugin", plural: "Plugins", icon: "package", tint: "teal" },
    settings: { label: "Settings", plural: "Settings", icon: "sliders", tint: "slate" },
    style: { label: "Output style", plural: "Output styles", icon: "palette", tint: "pink" },
    workflow: { label: "Workflow", plural: "Workflows", icon: "workflow", tint: "indigo" },
    claudemd: { label: "CLAUDE.md", plural: "CLAUDE.md", icon: "file-text", tint: "amber" },
  };

  const SOURCES = {
    addy: { label: "addyosmani/agent-skills", kind: "git", url: "https://github.com/addyosmani/agent-skills", branch: "main", subfolder: "/", commit: "aeac41d", version: "0.6.13" },
    matt: { label: "mattpocock/skills", kind: "git", url: "https://github.com/mattpocock/skills", branch: "main", subfolder: "/skills", commit: "a41c9e2", version: "a41c9e2" },
    pstack: { label: "pstack", kind: "git", url: "https://github.com/pstack-dev/pstack", branch: "main", subfolder: "/.claude", commit: "5e0b7aa", version: "3.2.0" },
    gist: { label: "gist.github.com", kind: "git", url: "https://gist.github.com/anon/9c01d2f", branch: "main", subfolder: "/", commit: "9c01d2f", version: "9c01d2f" },
    local: { label: "Local", kind: "local", url: "~/dotfiles/claude", branch: "", subfolder: "", commit: "", version: "local" },
  };

  const TAGS = [
    { id: "testing", color: "cyan" },
    { id: "review", color: "indigo" },
    { id: "frontend", color: "violet" },
    { id: "rust", color: "stone" },
    { id: "security", color: "slate" },
    { id: "planning", color: "sky" },
    { id: "daily", color: "plain" },
    { id: "experimental", color: "fuchsia" },
    { id: "docs", color: "ink" },
  ];

  // Deterministic pseudo-random so the demo looks the same on every reload.
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const pick = (arr) => arr[Math.floor(rnd() * arr.length)];
  const usage = (level) => Array.from({ length: 8 }, (_, i) => Math.max(0, Math.round(level * (0.55 + 0.08 * i + rnd() * 0.35))));

  const A = [];
  const add = (o) => {
    const a = Object.assign({
      invocation: o.type === "skill" || o.type === "subagent" ? "auto" : null,
      trust: 90, findings: [], bundle: [], tags: [], installedIn: [],
      reach: { files: "project files", shell: "none", network: "none", env: "none" },
      usage: usage(4), lastUsed: "3 days ago", latest: null, localEdits: false,
    }, o);
    a.sessions = a.usage.reduce((s, n) => s + n, 0);
    A.push(a);
    return a;
  };

  // ---- Detailed, hand-written artifacts --------------------------------
  add({ id: "tdd", name: "test-driven-development", type: "skill", source: "addy", desc: "Red-green-refactor loop for any logic change. Write a failing test, the minimum code to pass it, then refactor.", trust: 88, bundle: ["code-reviewer", "test-engineer", "incremental-implementation"], tags: ["testing", "daily"], installedIn: ["harneo", "my-api", "site", "billing"], reach: { files: "./src/**, ./tests/**", shell: "npm test, cargo test", network: "none", env: "none" }, usage: [4, 5, 5, 6, 7, 7, 9, 11], lastUsed: "today" });
  add({ id: "grill-me", name: "grill-me", type: "skill", source: "matt", desc: "Interviews you relentlessly about a plan until every branch of the decision tree is resolved.", trust: 92, invocation: "manual", tags: ["planning"], installedIn: ["harneo", "my-api", "site"], usage: [3, 2, 4, 3, 5, 4, 5, 5], lastUsed: "yesterday" });
  add({ id: "code-reviewer", name: "code-reviewer", type: "subagent", source: "addy", desc: "Senior reviewer across correctness, readability, architecture, security and performance.", trust: 90, tags: ["review"], installedIn: ["harneo", "my-api", "site", "billing"], reach: { files: "read-only: whole project", shell: "git diff", network: "none", env: "none" }, usage: [2, 3, 2, 3, 3, 3, 3, 3] });
  add({ id: "test-engineer", name: "test-engineer", type: "subagent", source: "addy", desc: "QA engineer for test strategy, writing tests and coverage analysis.", trust: 89, tags: ["testing"], installedIn: ["harneo", "my-api"], usage: [1, 2, 2, 2, 3, 2, 3, 3] });
  add({ id: "incremental-implementation", name: "incremental-implementation", type: "skill", source: "addy", desc: "Delivers changes in thin, verifiable slices: implement, test, verify, commit.", trust: 90, tags: ["daily"], installedIn: ["harneo", "my-api", "billing"], usage: [3, 4, 4, 5, 5, 6, 6, 7] });
  add({ id: "ship", name: "/ship", type: "command", source: "pstack", desc: "Runs the release checklist: changelog, version bump, tag and the pre-launch review.", trust: 84, invocation: "manual", latest: "3.4.1", tags: ["review"], installedIn: ["my-api", "billing"], reach: { files: "CHANGELOG.md, package.json", shell: "git tag, npm version", network: "none", env: "none" }, usage: [1, 2, 1, 1, 2, 1, 2, 2], lastUsed: "5 days ago" });
  add({ id: "strict-guard", name: "strict-guard", type: "settings", source: "local", desc: "Strict deny rules, the sensitive-path guard hook and the status line. Settings template.", trust: 95, invocation: null, tags: ["security"], installedIn: ["harneo", "my-api", "site", "billing", "docs-site"], reach: { files: ".claude/hooks/guard.py", shell: "python3 (hook)", network: "none", env: "none" }, usage: [0, 0, 0, 0, 0, 0, 0, 0], lastUsed: "—" });
  add({ id: "github", name: "github", type: "mcp", source: "local", desc: "GitHub MCP server over stdio. Token read from ${GITHUB_TOKEN} at runtime; never stored.", trust: 80, invocation: null, tags: ["daily"], installedIn: ["harneo", "my-api", "site", "billing", "docs-site", "cli-tools"], reach: { files: "none", shell: "npx @modelcontextprotocol/server-github", network: "api.github.com", env: "GITHUB_TOKEN" }, usage: [4, 5, 5, 4, 6, 5, 5, 6] });
  add({ id: "shell-runner", name: "shell-runner", type: "skill", source: "gist", desc: "Runs shell commands the model suggests, with curl piped to sh in its examples.", trust: 41, findings: [
    { level: "high", title: "Remote code execution pattern", detail: "SKILL.md line 14: `curl -fsSL $URL | sh`" },
    { level: "high", title: "Unrestricted shell", detail: "allowed-tools: Bash(*) with no command list" },
    { level: "medium", title: "Unpinned source", detail: "Gist has no tags; content can change silently" },
  ], tags: ["experimental"], installedIn: ["cli-tools"], reach: { files: "any", shell: "any command", network: "any host", env: "all" }, usage: [1, 0, 1, 0, 0, 1, 0, 0], lastUsed: "3 weeks ago" });
  add({ id: "webperf", name: "webperf", type: "skill", source: "addy", desc: "Web performance audit via the web-performance-auditor persona.", trust: 90, invocation: "manual", tags: ["frontend"], usage: [0, 0, 0, 0, 0, 0, 0, 0], lastUsed: "9 weeks ago" });
  add({ id: "rust-conventions", name: "rust-conventions", type: "rule", source: "local", desc: "Rust style rules, path-gated to **/*.rs: no unwrap outside tests, thiserror in libraries.", trust: 97, invocation: null, tags: ["rust"], installedIn: ["harneo", "cli-tools"], usage: [2, 2, 3, 2, 3, 3, 2, 3] });
  add({ id: "db-migrator", name: "db-migrator", type: "subagent", source: "pstack", desc: "Writes and checks expand/contract database migrations.", trust: 86, usage: [0, 0, 0, 0, 0, 0, 0, 0], lastUsed: "12 weeks ago" });
  add({ id: "spec-driven-development", name: "spec-driven-development", type: "skill", source: "addy", desc: "Writes a structured spec before any code: objective, commands, structure, style, testing, boundaries.", trust: 91, invocation: "manual", tags: ["planning", "docs"], installedIn: ["harneo"], usage: [1, 1, 2, 1, 2, 2, 3, 2] });
  add({ id: "frontend-ui-engineering", name: "frontend-ui-engineering", type: "skill", source: "addy", desc: "Production-quality, accessible, responsive UI: components, layout, state, WCAG.", trust: 90, tags: ["frontend"], installedIn: ["site"], usage: [2, 1, 2, 3, 2, 3, 3, 2] });
  add({ id: "security-and-hardening", name: "security-and-hardening", type: "skill", source: "addy", desc: "Hardens code against vulnerabilities: input handling, secrets, dependencies, OWASP Top Ten.", trust: 93, tags: ["security"], installedIn: ["my-api", "billing"], usage: [1, 1, 1, 2, 1, 2, 2, 2] });
  add({ id: "security-auditor", name: "security-auditor", type: "subagent", source: "addy", desc: "Security engineer for vulnerability detection and threat modeling.", trust: 92, tags: ["security", "review"], installedIn: ["billing"], usage: [1, 0, 1, 1, 1, 1, 1, 2] });
  add({ id: "build", name: "/build", type: "command", source: "addy", desc: "Implement the next task incrementally: build, test, verify, commit.", trust: 90, invocation: "manual", tags: ["daily"], installedIn: ["harneo", "my-api"], usage: [3, 3, 4, 4, 5, 4, 6, 6] });
  add({ id: "review-cmd", name: "/review", type: "command", source: "addy", desc: "Five-axis code review: correctness, readability, architecture, security, performance.", trust: 90, invocation: "manual", tags: ["review"], installedIn: ["harneo", "my-api", "site"], usage: [2, 2, 3, 2, 3, 3, 4, 3] });
  add({ id: "sentry", name: "sentry", type: "mcp", source: "local", desc: "Sentry MCP over HTTP; auth header from ${SENTRY_AUTH_TOKEN}.", trust: 82, invocation: null, tags: ["daily"], installedIn: ["my-api", "billing"], reach: { files: "none", shell: "none", network: "sentry.io", env: "SENTRY_AUTH_TOKEN" }, usage: [1, 1, 2, 1, 1, 2, 1, 2] });
  add({ id: "agent-skills-plugin", name: "agent-skills", type: "plugin", source: "addy", desc: "Addy Osmani's plugin, kept whole: 25 skills, 4 subagents, 9 commands, hooks.", trust: 89, invocation: null, tags: ["daily"], installedIn: ["harneo"], usage: [6, 7, 7, 8, 9, 9, 10, 12] });
  add({ id: "terse", name: "terse", type: "style", source: "local", desc: "Output style: short answers, exact paths, no preamble.", trust: 99, invocation: null, tags: ["daily"], installedIn: ["harneo", "cli-tools"], usage: [3, 3, 3, 3, 3, 3, 3, 3] });
  add({ id: "claudemd-rust", name: "rust-service CLAUDE.md", type: "claudemd", source: "local", desc: "CLAUDE.md template for Rust services: build commands, test policy, crate layout, @imports.", trust: 98, invocation: null, tags: ["rust", "docs"], installedIn: ["harneo"], usage: [0, 0, 0, 0, 0, 0, 0, 0], lastUsed: "—" });
  add({ id: "release-flow", name: "release", type: "workflow", source: "pstack", desc: "Workflow orchestrating the changelog, test-engineer and code-reviewer subagents.", trust: 85, invocation: null, latest: "3.4.1", localEdits: true, tags: ["review"], installedIn: ["billing"], usage: [0, 1, 0, 1, 1, 0, 1, 1] });
  add({ id: "write-a-prd", name: "write-a-prd", type: "skill", source: "matt", desc: "Turns a rough idea into a product requirements document through a short interview.", trust: 91, invocation: "manual", tags: ["planning", "docs"], installedIn: ["site"], usage: [1, 0, 1, 1, 0, 1, 1, 1] });

  // ---- Filler to reach a realistic library size (synthetic) ------------
  const counts = { skill: 96, subagent: 22, command: 18, rule: 12, mcp: 9, plugin: 8, settings: 6, style: 4, workflow: 3, claudemd: 6 };
  const words = {
    skill: ["debugging-and-error-recovery", "context-engineering", "api-and-interface-design", "code-simplification", "documentation-and-adrs", "git-workflow-and-versioning", "ci-cd-and-automation", "observability-and-instrumentation", "performance-optimization", "doubt-driven-development", "idea-refine", "interview-me", "deprecation-and-migration", "source-driven-development", "shipping-and-launch", "browser-testing-with-devtools", "constraint-driven-development", "planning-and-task-breakdown", "using-agent-skills", "refactor-safely", "explain-diff", "write-tests-first", "pr-description", "changelog-writer", "dependency-audit", "sql-review", "react-patterns", "tailwind-tidy", "rust-errors", "tauri-commands", "cli-ux", "readme-writer", "bug-repro", "commit-splitter", "type-narrowing", "accessibility-pass", "i18n-check", "monorepo-nav", "migration-planner", "feature-flags", "load-test", "log-triage", "incident-notes", "api-pagination", "schema-design", "cache-strategy", "naming-review", "dead-code", "flaky-test-hunt", "storybook-stories", "e2e-playwright", "docker-tidy", "k8s-manifests", "terraform-plan-read", "regex-explain", "perf-profile", "memory-leak-hunt", "csv-cleaner", "json-schema", "openapi-sync", "graphql-review", "auth-flows", "rate-limits", "retry-policies", "error-messages", "copy-edit", "design-tokens", "css-layout", "svg-optimize", "image-alt-text", "seo-basics", "analytics-events", "privacy-review", "license-check", "semver-bump", "release-notes", "grill-with-docs", "triage-issue", "to-prd", "improve-architecture", "write-a-skill", "prototype-fast", "code-tour", "test-matrix", "env-check", "pre-commit-review", "mock-data", "query-plans"],
    subagent: ["web-performance-auditor", "doc-writer", "migration-checker", "api-designer", "ux-reviewer", "dependency-bot", "perf-profiler", "a11y-auditor", "sql-optimizer", "release-manager", "triage-bot", "refactorer", "spec-checker", "copy-editor", "infra-reviewer", "data-modeler", "bug-hunter", "test-writer"],
    command: ["/spec", "/plan", "/test", "/code-simplify", "/constraints", "/webperf", "/standup", "/pr", "/fixup", "/bench", "/deps", "/todo", "/explain", "/onboard", "/triage", "/retro"],
    rule: ["ts-strict", "no-console", "commit-style", "api-errors", "react-hooks", "sql-migrations", "logging", "secrets-policy", "test-naming", "docs-tone", "css-tokens"],
    mcp: ["linear", "postgres", "filesystem-ro", "playwright", "slack", "notion", "fetch"],
    plugin: ["impeccable", "session-radar", "waiting-bell", "pstack-core", "review-kit", "mods-pack", "docs-kit"],
    settings: ["relaxed", "ci-bot", "readonly-audit", "team-default", "solo-fast"],
    style: ["explanatory", "review-notes", "changelog"],
    workflow: ["triage", "onboarding"],
    claudemd: ["node-api CLAUDE.md", "react-app CLAUDE.md", "library CLAUDE.md", "monorepo CLAUDE.md", "docs-site CLAUDE.md"],
  };
  const DESC = {
    "debugging-and-error-recovery": "Root-causes a failure before changing code: reproduce, bisect, fix, add the test.",
    "context-engineering": "Sets up rules files and context so agents get the right information at the right time.",
    "api-and-interface-design": "Contract-first interface design: error semantics, versioning, boundary validation.",
    "code-simplification": "Reduces complexity without changing behaviour; deletes before it adds.",
    "documentation-and-adrs": "Writes ADRs and docs that record why a decision was made, not just what.",
    "git-workflow-and-versioning": "Atomic commits, clean history, semantic version bumps and changelogs.",
    "ci-cd-and-automation": "Builds CI pipelines with quality gates that block on real failures only.",
    "observability-and-instrumentation": "Adds logs, metrics and traces so production behaviour is diagnosable.",
    "performance-optimization": "Profiles first, then fixes the measured bottleneck; no speculative tuning.",
    "doubt-driven-development": "Cross-examines every non-trivial decision in a fresh context before it stands.",
    "idea-refine": "Turns a vague idea into a sharp concept through divergent then convergent thinking.",
    "interview-me": "Asks one question at a time until the real intent behind a request is clear.",
    "deprecation-and-migration": "Plans expand/contract migrations and deprecations without downtime.",
    "source-driven-development": "Checks every framework call against the official docs and cites them.",
    "shipping-and-launch": "Pre-launch checklist: monitoring, rollback plan, staged rollout.",
    "browser-testing-with-devtools": "Verifies UI in a real browser: DOM, console errors, network, performance.",
    "constraint-driven-development": "Writes the quality bar down as CONSTRAINTS.md and guards it against erosion.",
    "planning-and-task-breakdown": "Breaks a spec into small, ordered tasks with acceptance criteria.",
    "using-agent-skills": "Picks the right skill for the work in front of you; the pack's index.",
    "refactor-safely": "Refactors behind passing tests in small steps, one rename or move per commit.",
    "explain-diff": "Explains a diff in plain words: what changed, why, and what could break.",
    "write-tests-first": "Writes the failing test from the bug report before touching the code.",
    "pr-description": "Drafts a PR description from the diff: summary, risk, how to test.",
    "changelog-writer": "Turns merged commits into a user-facing changelog entry.",
    "dependency-audit": "Lists outdated and vulnerable dependencies with the safest upgrade path.",
    "sql-review": "Reviews queries for missing indexes, N+1 patterns and unsafe migrations.",
    "react-patterns": "React conventions: state colocation, effects only for sync, stable keys.",
    "tailwind-tidy": "Sorts and dedupes Tailwind classes; extracts repeated groups into components.",
    "rust-errors": "thiserror for libraries, anyhow for binaries, no unwrap outside tests.",
    "tauri-commands": "Tauri 2 IPC commands: typed payloads, thin handlers, logic in the core crate.",
    "cli-ux": "CLI conventions: helpful errors, --json output, exit codes, no surprise prompts.",
    "readme-writer": "Writes a README that gets a newcomer from clone to running in five minutes.",
    "bug-repro": "Builds the smallest reproduction of a bug before anyone attempts a fix.",
    "commit-splitter": "Splits a messy working tree into clean, reviewable commits.",
    "type-narrowing": "Replaces casts and any with proper narrowing and discriminated unions.",
    "accessibility-pass": "Checks keyboard paths, focus order, labels and contrast on changed screens.",
    "i18n-check": "Finds hard-coded strings and layouts that break with longer translations.",
    "monorepo-nav": "Maps packages and their owners before changing anything in a monorepo.",
    "migration-planner": "Plans a schema migration as reversible steps with a rollback for each.",
    "feature-flags": "Ships risky changes behind a flag with a removal date.",
    "load-test": "Writes a k6 load test for an endpoint and reads the latency percentiles.",
    "log-triage": "Groups error logs by root cause and ranks them by user impact.",
    "incident-notes": "Keeps a timestamped incident log and drafts the blameless postmortem.",
    "api-pagination": "Cursor-based pagination with stable ordering and clear limits.",
    "schema-design": "Models data around access patterns; flags fields that will need migrations.",
    "cache-strategy": "Chooses what to cache, where, and how it gets invalidated.",
    "naming-review": "Reviews names for clarity and consistency across a change.",
    "dead-code": "Finds unreachable code and unused exports, and proves they're unused first.",
    "flaky-test-hunt": "Reruns a suite to find flaky tests and fixes their cause, not the symptom.",
    "storybook-stories": "Writes stories for every component state, including empty and error.",
    "e2e-playwright": "End-to-end tests with Playwright against real user flows.",
    "docker-tidy": "Smaller images: multi-stage builds, pinned bases, nothing sensitive in layers.",
    "k8s-manifests": "Reviews Kubernetes manifests for limits, probes and security context.",
    "terraform-plan-read": "Reads a terraform plan and flags destructive or surprising changes.",
    "regex-explain": "Explains a regular expression piece by piece, with test cases.",
    "perf-profile": "Captures and reads a CPU profile to find the hot path.",
    "memory-leak-hunt": "Tracks down growing heaps with snapshots and retention paths.",
    "csv-cleaner": "Normalizes messy CSV: headers, encodings, dates and duplicates.",
    "json-schema": "Writes JSON Schema for a payload and validates samples against it.",
    "openapi-sync": "Keeps the OpenAPI spec in sync with the actual routes.",
    "graphql-review": "Reviews GraphQL schemas for N+1 resolvers and breaking changes.",
    "auth-flows": "Session, OAuth and token flows with the usual pitfalls called out.",
    "rate-limits": "Adds rate limiting with sensible defaults and clear 429 responses.",
    "retry-policies": "Retries with backoff and jitter, only on errors that are safe to retry.",
    "error-messages": "Rewrites error messages to name the problem and the way out.",
    "copy-edit": "Tightens UI copy: shorter labels, verbs on buttons, no jargon.",
    "design-tokens": "Moves raw colours and sizes into tokens, light and dark.",
    "css-layout": "Fixes layouts with grid and flex instead of magic numbers.",
    "svg-optimize": "Shrinks SVGs and makes them inherit currentColor.",
    "image-alt-text": "Writes useful alt text, and marks decorative images as such.",
    "seo-basics": "Titles, meta descriptions, canonical URLs and structured data.",
    "analytics-events": "Defines analytics events with names, properties and an owner.",
    "privacy-review": "Flags personal data flows and where consent or deletion is needed.",
    "license-check": "Lists dependency licences and flags incompatible ones.",
    "semver-bump": "Picks the right semver bump from the changes since the last tag.",
    "release-notes": "Writes release notes for users, not for the team.",
    "grill-with-docs": "Interviews you about a plan while checking claims against the repo's docs.",
    "triage-issue": "Turns a raw bug report into a reproducible, labelled issue.",
    "to-prd": "Converts a conversation into a product requirements document.",
    "improve-architecture": "Proposes module boundaries that make the codebase easier to change.",
    "write-a-skill": "Scaffolds a new Claude Code skill with frontmatter and examples.",
    "prototype-fast": "Builds a throwaway prototype to answer one question, then deletes it.",
    "code-tour": "Walks a newcomer through the codebase in the order a request flows.",
    "test-matrix": "Lists the cases a change must be tested against before writing tests.",
    "env-check": "Verifies required tools and environment variables before a build.",
    "pre-commit-review": "Reviews staged changes for leftovers: debug logs, TODOs, tokens.",
    "mock-data": "Generates realistic fixture data that respects the schema.",
    "query-plans": "Reads EXPLAIN output and suggests the index that fixes it.",
    "web-performance-auditor": "Audits Core Web Vitals and names the change that moves each one.",
    "doc-writer": "Writes and updates docs alongside code changes.",
    "migration-checker": "Reviews database migrations for locks and irreversible steps.",
    "api-designer": "Designs endpoints and payloads before implementation starts.",
    "ux-reviewer": "Reviews screens for clarity, states and keyboard paths.",
    "dependency-bot": "Opens small, tested dependency upgrades one at a time.",
    "perf-profiler": "Profiles slow paths and reports the measured bottleneck.",
    "a11y-auditor": "Audits pages against WCAG AA and lists fixes by impact.",
    "sql-optimizer": "Rewrites slow queries and proposes indexes.",
    "release-manager": "Coordinates the release: changelog, tag, notes, rollback plan.",
    "triage-bot": "Labels and routes incoming issues.",
    "refactorer": "Carries out mechanical refactors across many files safely.",
    "spec-checker": "Compares an implementation against its spec and lists gaps.",
    "copy-editor": "Edits user-facing text for tone and length.",
    "infra-reviewer": "Reviews infrastructure changes for cost and blast radius.",
    "data-modeler": "Designs tables and relations from the access patterns.",
    "bug-hunter": "Reads a diff looking only for bugs, with a failing case for each.",
    "test-writer": "Adds missing tests for existing behaviour before a change.",
    "/spec": "Starts spec-driven development: writes the spec before any code.",
    "/plan": "Breaks the approved spec into ordered tasks.",
    "/test": "Runs the TDD loop for the current task.",
    "/code-simplify": "Simplifies recently changed code without changing behaviour.",
    "/constraints": "Sets up or checks the project's quality bar.",
    "/webperf": "Runs a web performance audit.",
    "/standup": "Summarizes yesterday's commits and today's open tasks.",
    "/pr": "Opens a pull request with a generated description.",
    "/fixup": "Creates fixup commits for review feedback.",
    "/bench": "Runs benchmarks and compares them with main.",
    "/deps": "Checks for outdated and vulnerable dependencies.",
    "/todo": "Lists TODO and FIXME comments by file.",
    "/explain": "Explains the selected code.",
    "/onboard": "Gives a newcomer a guided tour of the repo.",
    "/triage": "Triages new issues.",
    "/retro": "Drafts a sprint retrospective from merged PRs.",
    "ts-strict": "TypeScript rules: strict mode, no any, no non-null assertions.",
    "no-console": "No console.log in committed code; use the logger.",
    "commit-style": "Commit messages: imperative summary, body explains why.",
    "api-errors": "API errors return a code, a message and a hint.",
    "sql-migrations": "Migrations are reversible and never lock large tables.",
    "logging": "Structured logs with request IDs; never log tokens or passwords.",
    "secrets-policy": "Credentials come from the environment, never from files in the repo.",
    "test-naming": "Test names describe the behaviour, not the function.",
    "docs-tone": "Docs are short, direct and task-first.",
    "css-tokens": "Components use design tokens only; no raw hex or px.",
    "linear": "Linear MCP server; token from ${LINEAR_API_KEY}.",
    "postgres": "Postgres MCP server, read-only role; URL from ${DATABASE_URL}.",
    "filesystem-ro": "Read-only filesystem MCP for a docs folder.",
    "playwright": "Playwright MCP for driving a real browser.",
    "slack": "Slack MCP server; token from ${SLACK_BOT_TOKEN}.",
    "notion": "Notion MCP server; token from ${NOTION_TOKEN}.",
    "fetch": "Fetches web pages as markdown.",
    "impeccable": "Impeccable design plugin, kept whole: one skill, 24 commands, hooks.",
    "session-radar": "Shows other running Claude sessions in the status line.",
    "waiting-bell": "Rings when Claude is waiting for you.",
    "pstack-core": "pstack's core plugin: commands, agents and hooks.",
    "review-kit": "Review agents and commands bundled as one plugin.",
    "mods-pack": "Small quality-of-life mods for Claude Code.",
    "docs-kit": "Doc-writing skills and an output style.",
    "relaxed": "Settings template: few prompts, broad allow list, no hooks.",
    "ci-bot": "Settings template for CI: no prompts, network off, strict deny list.",
    "readonly-audit": "Settings template: read-only tools, for audits.",
    "team-default": "Settings template shared by the team: guard hook and status line.",
    "solo-fast": "Settings template for solo work: allow common dev commands.",
    "explanatory": "Output style: explains each step for someone learning.",
    "review-notes": "Output style: findings as a numbered list with severity.",
    "changelog": "Output style: writes like a changelog entry.",
    "triage": "Workflow: labels issues, reproduces bugs, routes them.",
    "onboarding": "Workflow: tours the repo and sets up the dev environment.",
    "node-api CLAUDE.md": "CLAUDE.md for Node APIs: scripts, test policy, env setup.",
    "react-app CLAUDE.md": "CLAUDE.md for React apps: dev server, components, testing.",
    "library CLAUDE.md": "CLAUDE.md for libraries: public API rules, semver, docs.",
    "monorepo CLAUDE.md": "CLAUDE.md for monorepos: package map and per-package commands.",
    "docs-site CLAUDE.md": "CLAUDE.md for docs sites: content rules and build steps."
  };

  const descs = {
    skill: "Skill for %s work, with its own scripts and examples.",
    subagent: "Subagent focused on %s.",
    command: "Slash command that runs %s.",
    rule: "Rule file for %s, path-gated where relevant.",
    mcp: "MCP server for %s; secrets only as ${VAR}.",
    plugin: "Plugin kept whole: %s.",
    settings: "Settings template: %s permissions, hooks and status line.",
    style: "Output style: %s.",
    workflow: "Workflow orchestrating subagents for %s.",
    claudemd: "CLAUDE.md template: %s.",
  };
  Object.keys(counts).forEach((type) => {
    const list = words[type];
    let i = 0;
    while (A.filter((a) => a.type === type).length < counts[type]) {
      const base = list[i % list.length] + (i >= list.length ? "-" + (Math.floor(i / list.length) + 1) : "");
      i++;
      if (A.some((a) => a.name === base)) continue;
      const src = pick(type === "skill" || type === "subagent" || type === "command" ? ["addy", "addy", "pstack", "pstack", "matt", "local", "local"] : ["local", "local", "pstack"]);
      const level = rnd() < 0.13 ? 0 : 1 + Math.floor(rnd() * 5);
      const tagPool = TAGS.map((t) => t.id);
      add({
        id: "f-" + type + "-" + i,
        name: base,
        type,
        source: src,
        desc: DESC[base] || descs[type].replace("%s", base.replace(/[-/]/g, " ").replace(/^ /, "").trim()),
        trust: 72 + Math.floor(rnd() * 27),
        invocation: type === "skill" || type === "subagent" ? (rnd() < 0.25 ? "manual" : "auto") : null,
        usage: level === 0 ? [0, 0, 0, 0, 0, 0, 0, 0] : usage(level),
        lastUsed: level === 0 ? (8 + Math.floor(rnd() * 8)) + " weeks ago" : pick(["today", "yesterday", "2 days ago", "4 days ago", "last week"]),
        latest: src === "pstack" && rnd() < 0.2 ? "3.4.1" : null,
        tags: rnd() < 0.6 ? [pick(tagPool)] : [],
      });
    }
  });

  const PROFILES = [
    { id: "heavy", name: "Heavy", version: 4, desc: "Full factory: TDD, review, security, release.", settings: "strict-guard", mcp: [{ id: "github", env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } }, { id: "sentry", env: { SENTRY_AUTH_TOKEN: "${SENTRY_AUTH_TOKEN}" } }], items: ["tdd", "code-reviewer", "test-engineer", "incremental-implementation", "grill-me", "build", "review-cmd", "ship", "security-and-hardening", "terse"], projects: ["my-api", "billing"] },
    { id: "light", name: "Light", version: 2, desc: "Small projects: TDD and review only.", settings: "strict-guard", mcp: [{ id: "github", env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } }], items: ["tdd", "code-reviewer", "test-engineer", "incremental-implementation", "review-cmd", "grill-me"], projects: ["site", "docs-site"] },
    { id: "rust", name: "Rust service", version: 3, desc: "Rust conventions, CLAUDE.md template, Addy's plugin.", settings: "strict-guard", mcp: [{ id: "github", env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } }], items: ["tdd", "code-reviewer", "test-engineer", "incremental-implementation", "rust-conventions", "claudemd-rust", "agent-skills-plugin", "spec-driven-development", "terse"], projects: ["harneo"] },
    { id: "frontend", name: "Frontend", version: 1, desc: "UI work: frontend engineering and review.", settings: "strict-guard", mcp: [], items: ["frontend-ui-engineering", "code-reviewer", "write-a-prd", "tdd", "test-engineer", "incremental-implementation"], projects: [] },
    { id: "audit", name: "Audit only", version: 1, desc: "Read-only review and security audit.", settings: "strict-guard", mcp: [], items: ["security-auditor", "code-reviewer", "shell-runner"], projects: ["cli-tools"] },
  ];

  const PROJECTS = [
    { id: "harneo", name: "harneo", path: "~/projects/harneo", profile: "rust", version: 3, status: "sync", applied: "2 days ago", backups: 4 },
    { id: "my-api", name: "my-api", path: "~/work/my-api", profile: "heavy", version: 4, status: "drift", drift: [
      { path: ".claude/agents/code-reviewer.md", change: "edited in project", plus: 6, minus: 2 },
      { path: ".claude/commands/test.md", change: "deleted in project", plus: 0, minus: 31 },
      { path: ".claude/settings.json", change: "edited in project", plus: 3, minus: 0 },
    ], applied: "3 weeks ago", backups: 7 },
    { id: "site", name: "site", path: "~/work/site", profile: "light", version: 1, status: "outdated", applied: "2 months ago", backups: 2 },
    { id: "billing", name: "billing", path: "~/work/billing", profile: "heavy", version: 4, status: "sync", applied: "5 days ago", backups: 5 },
    { id: "docs-site", name: "docs-site", path: "~/work/docs-site", profile: "light", version: 2, status: "sync", applied: "last week", backups: 1 },
    { id: "cli-tools", name: "cli-tools", path: "~/projects/cli-tools", profile: "audit", version: 1, status: "sync", applied: "3 weeks ago", backups: 1 },
  ];

  const COLLECTIONS = [
    { id: "c-rust", name: "Rust tooling", kind: "smart", rule: { tags: ["rust"] } },
    { id: "c-cleanup", name: "To clean up", kind: "smart", rule: { status: ["unused", "outdated"] } },
    { id: "c-review", name: "My review kit", kind: "manual", items: ["code-reviewer", "review-cmd", "security-auditor", "grill-me"] },
  ];

  // What a scan of ~/.claude finds on first run / import (synthetic).
  const SCAN = {
    claudeHome: [
      { name: "tdd-lite", type: "skill", note: "~/.claude/skills/tdd-lite" },
      { name: "commit-helper", type: "command", note: "~/.claude/commands/commit-helper.md" },
      { name: "reviewer", type: "subagent", note: "~/.claude/agents/reviewer.md" },
      { name: "personal-settings", type: "settings", note: "~/.claude/settings.json + 2 hook scripts" },
      { name: "terse", type: "style", note: "~/.claude/output-styles/terse.md" },
      { name: "github", type: "mcp", note: "~/.claude.json mcpServers.github" },
    ],
    git: [
      { name: "test-driven-development", type: "skill", note: "skills/test-driven-development" },
      { name: "spec-driven-development", type: "skill", note: "skills/spec-driven-development" },
      { name: "code-review-and-quality", type: "skill", note: "skills/code-review-and-quality" },
      { name: "incremental-implementation", type: "skill", note: "skills/incremental-implementation" },
      { name: "frontend-ui-engineering", type: "skill", note: "skills/frontend-ui-engineering" },
      { name: "security-and-hardening", type: "skill", note: "skills/security-and-hardening" },
      { name: "code-reviewer", type: "subagent", note: "agents/code-reviewer.md" },
      { name: "test-engineer", type: "subagent", note: "agents/test-engineer.md" },
      { name: "/build", type: "command", note: "commands/build.toml" },
      { name: "/review", type: "command", note: "commands/review.toml" },
    ],
  };

  window.HARNEO_DATA = { TYPES, SOURCES, TAGS, ARTIFACTS: A, PROFILES, PROJECTS, COLLECTIONS, SCAN };
})();
