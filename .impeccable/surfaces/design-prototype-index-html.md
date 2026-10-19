---
version: 1
slug: "design-prototype-index-html"
primary_target: "design/prototype/index.html"
related_targets: []
---

# Surface brief: Harneo v1 clickable prototype

Scope: the whole v1 desktop app (`CONTEXT.md` §3–4) as a static clickable prototype in `design/prototype/`, realistic synthetic data, no persistence. Visitor mode: Operate.

Audience and job: first-time developers who found Harneo on GitHub, plus the maintainer; short task-driven visits (import, build a profile, scaffold, refit, check drift, update, prune, organize with tags and collections). They must understand library, bundle, profile, refit and collection from the interface alone.

Screens: first run with guided import; Library home (cards ⇄ list, filters incl. tags, health, activity, detail panel); import; update with local edits; profiles and the profile builder (cart with live folder tree); projects and project detail (drift, switch, removal); scaffold and refit with dry-run previews and red loss warnings; cleanup; collections in the sidebar; command palette; settings. Light and dark (OS + toggle); full width and ~960px; full keyboard path.

## Direction contract

THESIS: The category standard at full craft, the way Vercel and Linear do it: a quiet white-and-slate working tool whose cards carry real proof (trust, reach, usage, status) instead of a bare table; refuses both the grey admin table and the neon dev-console costume.

OWN-WORLD: White ground, slate neutrals (#0F172A ink, #475569/#64748B secondary, #E2E8F0 hairlines), one blue (#2563EB) for selection and primary actions; state colours only where they mean something (green current, amber outdated, red findings or loss); unused artifacts recede to grey. Inter for UI, JetBrains Mono for paths, versions and counts; Lucide icons; 8px radius cards with hairline borders; per-type icon tints.

STORY: The visitor sees what they own, whether it is healthy and used, finds anything in seconds through filters, tags and collections, then composes profiles and installs them into projects, seeing every file before it is written and every line before it is lost.

FIRST VIEWPORT: Left sidebar (logo, Library / Profiles / Projects, Collections, actions, Ctrl K); filter column (type, tags, status, source, trust, invocation); main: title, search, cards/list switch, Import as the primary action top right; a Library health stacked bar and an 8-week activity chart; then a 3-column card grid; the detail panel slides over from the right.

FORM: Category standard (canon), taken by the user after the direction round; cards, tags and collections are user-pinned. seed:4cb87848e665db80

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
