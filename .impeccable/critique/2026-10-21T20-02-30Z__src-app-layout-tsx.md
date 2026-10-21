---
target: "app frame (#22)"
total_score: 24
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 2
target_identity: "file:/home/ibra/projects/harneo/src/app/Layout.tsx"
target_fingerprint: "sha256:6ca2fc5d64e9fde48ae74f43ab6088f6522e32d2e1470f3002e3ce944498e782"
target_path: /home/ibra/projects/harneo/src/app/Layout.tsx
timestamp: 2026-10-21T20-02-30Z
slug: src-app-layout-tsx
---
Method: dual-agent (A: design review · B: detector). Run notes: Tauri on Linux (WebKitGTK) exposes no CDP, so B ran the source scan plus supplementary Chromium captures of the built frontend; A reviewed source and captures.

Score 24/40 (all ten heuristics apply): status 3, real world 2, control 3, consistency 3, error prevention 3, recognition 2, efficiency 2, minimalist 3, error recovery 2, help 1.

Specificity: category standard executed faithfully (the user's chosen direction); Harneo's character lives in proof devices not yet built. Detector: CLI 0 findings on src/; in-page icon-tile-stack on the EmptyState icon tile (pattern inherited from the validated prototype) and one on the Collections heading at 960 (false positive).

Priority issues:
[P1] Palette input shows a clipped blue focus box: global :focus-visible in tokens.css overrides the module's outline: none.
[P1] Placeholder copy speaks the build plan ("arrives with the X module") instead of defining each place for first-time users.
[P2] Icon rail at <=1000px loses all grouping; names only via title tooltips, none on keyboard focus.
[P2] Primitives contradict DESIGN.md: Tag allows reserved state hues (green, red, amber, blue) and lacks plain/ink; extra --tint-green/red tokens; About panel version/path not in mono.
[P2] Palette thinner than the prototype: no group headers, Settings and shortcuts missing, no hover tracking, no scroll-into-view, "No matches." without the query.

Minor: screen/empty-state padding below DESIGN.md; double rule in shortcuts footer; button text sizes; danger-solid text token; tag padding; duplicate --mono/--font-mono; focus ring radius 4px vs 6px items; no skip link; brand and Library both aria-current; shortcuts dialog first focus invisible; Ctrl shown on macOS.
