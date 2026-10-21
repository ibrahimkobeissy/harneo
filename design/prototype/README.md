# Harneo clickable prototype

A static, clickable mockup of the whole Harneo v1 app, built to test the design and every flow before any app code exists ([#7](https://github.com/ibrahimkobeissy/harneo/issues/7)). Designed with Impeccable: the category standard (white, slate, one blue) at full craft, benchmarked on Vercel and Linear.

**Open it:** open `index.html` in a browser (no build, no server). Fonts and Lucide icons load from the network.

**Prototype controls** (bottom of the sidebar): theme toggle, *start as a first-time user* (empty library → welcome → guided import), *reset demo data*. State lives in memory; a reload resets it.

## What to try

| v1 feature (`CONTEXT.md` §3–4) | Where |
|---|---|
| First run, guided import | Sidebar → person-plus icon, then *Import your first artifacts* |
| Import from `~/.claude`, git URL, folder | *Import*; try `nope/does-not-exist` and `~/code/empty-dir` for the error states |
| AI bundle detection + tag suggestions + security audit | Import, step 3 *Review*; Settings → simulate the `claude` CLI missing |
| Library: cards ⇄ list, filters, health, activity, pagination | *Library* (home); every list column sorts on click |
| Tags (add, create, remove) | Click any card → *Tags* in the detail panel |
| Collections: smart (a saved rule) and manual (a list you pick) | Sidebar *Collections* → +: the dialog explains both and builds a smart rule in place; *Add to… → collection* in the detail panel |
| Artifact detail: provenance, trust, findings, reach, bundle, usage | Click a card (`shell-runner` shows findings); open a bundle member, then *Back* |
| Security audit: trust distribution, findings by severity, permission transparency | Sidebar *Security*; click a reach tile to filter |
| Settings templates: contents, view `settings.json`, duplicate, pick one per profile | Profile builder → *Settings template* (eye icon, *Manage*); Library → Type: Settings → any template |
| Invocation mode (manual only) | Detail panel toggle, or the profile builder's file view |
| Update + protect local edits | `/ship` (plain update) and `release` (local edits) → *Update* |
| Promote to library | Project `my-api` → Drift → *Promote to library* |
| Profiles: create, clone, export, import | *Profiles* |
| Profile builder: bundles pulled in, Settings template, MCP `${VAR}` only, live folder tree, totals | *Profiles* → any profile; type a raw token in an MCP field to see the guard |
| Create profile from collection | Open a collection → *Create profile* |
| Scaffold with dry-run preview, `HARNESS.md` | *Scaffold project*; `~/work/site` shows the "already has a harness" state, `~/readonly/demo` a write failure |
| Refit: keep / replace / delete, red loss warnings, diff preview, backups, restore | *Refit project* → `~/work/site` |
| Project registry, drift, re-apply, switch profile, clean removal | *Projects* → `my-api` (drift), `site` (profile upgrade) |
| Cleanup suggestions | *Cleanup*, or *Review N issues* on the Library health panel |
| Command palette, keyboard path | `Ctrl K`; *Keyboard shortcuts* in the sidebar or `?`; arrows + Enter move through cards |
| Light / dark, half-screen width | Theme toggle; resize to ~960px |

Left out on purpose: the profile *harness check* (not scoped yet, `CONTEXT.md` §6).

## Synthetic data

Artifact names from `addyosmani/agent-skills` are real. Everything else is invented for the demo and must not be read as fact: the other packs' contents, all trust scores and findings, usage numbers, projects, profiles, versions and file contents. The logo is a first draft.

## Files

`index.html` · `styles.css` (tokens + components, light and dark) · `data.js` (demo data) · `core.js` (state, router, frame, palette, keyboard, dialogs) · `library.js` · `profiles.js` · `projects.js` (+ cleanup, settings, welcome) · `flows.js` (import, scaffold, refit) · `security.js` (Security page, settings template contents).
