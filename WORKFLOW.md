# Workflow log

How Harneo is built with Addy Osmani's agent-skills and Impeccable: one line per step, in the order we ran them. Updated every time we go through a step.

1. `/spec` (step 0, capability map): split the product into 11 modules with a build order → `spec/capabilities.md`
2. `/spec`: wrote and approved the first module spec, which also sets project-wide conventions → `spec/app-shell.md`
3. `/constraints`: 4-question interview on the quality bar (what to enforce, block or warn, numbers, check budgets) → `CONSTRAINTS.md`
4. `/plan`: drafted 13 tasks in 3 phases with checkpoints; paused before creating issues, to see the design first → `tasks/plan.md` (draft)
5. `/impeccable init`: product context for the designer (users, purpose, tone, constraints) → `PRODUCT.md`
6. `/impeccable shape`: UX discovery (home, first run, navigation, library size, window, keyboard), then the visual direction round; the user took the category standard with cards, benchmarked on Vercel and Linear, and added tags and collections → the confirmed brief, `PRODUCT.md` visual preference, `CONTEXT.md` updates
7. `/impeccable` build (code-led): direction contract written, prototype built and click-tested, finish review (fix → 2 verdict rounds, one small item left open), documenter → `design/prototype/`, `DESIGN.md`, `.impeccable/design.json`
8. Prototype round 2 (user feedback, within the established design): pagination, sortable columns, Security page, settings template contents, smart vs manual collections, back link in bundles, filter column scroll, shortcuts button; checked against `DESIGN.md` by Impeccable's detector → `design/prototype/`
