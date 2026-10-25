---
target: "bundles + updates + usage + settings screens (#90, #93, #94)"
total_score: 26
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:src/features/import"
timestamp: 2026-10-23T15-05-40Z
slug: src-features-bundles-updates
---
⚠️ DEGRADED: single-context (the run was itself a sub-agent with the "do the work directly" brief, so A and B ran back to back in one context: A from screenshots and code first, B from the detector and measurements second). Tauri/WebKitGTK exposes no CDP; the built frontend was rendered in headless Chromium with stubbed IPC, light and dark at 1440 and 960. The detector ran through `--cdp` against that Chromium (36 page states).

## Design Health Score: 26/40 (Acceptable)

| # | Heuristic | Score | Key issue |
|---|---|---|---|
| 1 | System status | 3 | Progress bar is an empty track until the first batch reports; Import is disabled during detection with no reason or skip |
| 2 | Real world | 2 | "+ 2" with no verb, "reference / Claude / manual", "1/2 refs", "scan warnings" next to "Update", raw "claude exited with status 1" |
| 3 | Control | 3 | Switching a proposal off is remembered forever, silently; link removal has no undo |
| 4 | Consistency | 2 | Two primary buttons in the toolbar; amber means both Update and scan warning; Bundle badge is blue (DESIGN: neutral); Settings mixes a switch and a checkbox and two panel styles |
| 5 | Error prevention | 3 | Keep mine is the default, edited artifacts are skipped by Update all; but Update all can push its own buttons off-screen, and the whole-library Claude run starts with no confirm |
| 6 | Recognition | 3 | Card footer carries up to six items, the version appears twice |
| 7 | Efficiency | 2 | Find bundles hides in a one-item "..." menu; list view truncates Type, Source, Imported and clips Bundle |
| 8 | Minimalism | 2 | Card foot overload; empty "Needs (0) / Needed by (0)" blocks; two insight panels push the first card to y=456 at 960 |
| 9 | Error recovery | 3 | Retry and per-source errors are good; CLI-missing and detection-off are dead ends with no link to Settings or install help |
| 10 | Help | 3 | First-meeting copy ("installed together, always", "Nothing is saved until you accept") works; where to re-run detection is never said |

## Design Specificity
Authored for Harneo: the Review step's proposed-bundle groups, the "Yours vs Upstream" never-merge dialog, mono source lines with `-> 3.4.1`, session bars fed from Claude's logs and the update callout all read as this product (terse, technical, proof over trust). The weakness is accumulation: three features (bundles, updates, usage) each added badges and columns to the same card and table without a shared budget, so the Library card foot and list view now read like a debug strip.

Detector (CLI on source): 2 findings, both `design-system-radius` 3px off the 4px scale (Import.module.css:958, FindBundles.module.css:46, the progress bars). Detector (CDP, 36 states): Local-edit dialog light at 1440 and 960: 3 `low-contrast` (line numbers `.no` #64748b on diff washes, 4.2 to 4.3:1) and 1 `skipped-heading` (h2 to h4, both themes). Review step dark: 3 to 4 `ai-color-palette` (cyan and violet tag chips): false positive, they are the user-pickable tag palette that DESIGN.md documents. Library, list, drawer, settings, update dialog and Find dialog: clean. Measured (light, computed): proposal group switched off `.why` 2.8:1 and chip 3.1:1 (opacity 0.6); chip remove icon on a tag 2.8:1 at 12x12px; switch off track 1.5:1 against its row (DESIGN says `sunk`, code uses `--line2`); Update, scan-warning and version badges 4.51:1 (pass, no headroom). Dark all pass except the off-group text 3.7:1.

## Priority Issues
1. [P1] Update all dialog overflows: UpdateDialogs.tsx:525 renders the names list outside the `.scroll` wrapper. With 61 rows (stub) the list is 2,237px tall inside an 80vh modal and the "Update N" button sits at y=2521 on a 900px viewport; with about 14 or more outdated artifacts the confirm and Cancel are unreachable by eye.
2. [P1] List view is clipped after #90/#93 added Sessions, Status and Bundle columns: at 1440 with the filter column the table is 936px wide; Type shows "Subage..." and "CLAU...", Source "addyosman...", Imported "Oct 21, 2...", Tags "fron", the Name cell wraps the icon and scan badges onto three lines, and "Bundle of 1" is cut at the right edge.
3. [P1] Proposal controls fail accessibility: off-state text 2.8 to 3.1:1 (opacity .6), the switch off track is 1.5:1 and carries no visible On/Off word, and the remove x on bundle members and tags is 12x12px at 0.7 opacity.
4. [P2] Library card footer overload and colour collisions (version said twice, amber for update and for scan warning, blue Bundle badge, two primary buttons in the toolbar).
5. [P2] Import Review and Find bundles do not say what accepting does ("+ 2", default-accepted, a switched-off group is remembered forever, nothing on the Import button, confidence ignored).
6. [P2] Detection dead ends: CLI missing, detection off, error and loading give no next step.
7. [P2] Bundle section and Settings polish (origin chips, note collides with a reason, empty double headings, Settings inconsistency, no install or re-check).

## Persona red flags
Alex (power user): Find bundles is only in the "..." menu; no accept-all or "skip detection" during the run; list view truncates exactly the columns used to scan; Update all cannot be confirmed on a long list.
Jordan (first-timer who has never heard of a bundle): "+ 2" has no noun or verb; groups are on by default so "Import 6 artifacts" silently stores links; "reference" vs "Claude" vs "manual" is undefined; CLI-not-found says "re-run detection later" but not where.
Sam (keyboard and AT): switch track 1.5:1 and no On/Off text; 12px remove targets; switched-off groups below AA; "Needs (0)" empty sections announce noise; tooltips (`title`) are the only per-week values in the usage bars.

## Minor
Progress bar has `max=1 value=0` until the first batch event (use indeterminate); 3px bar radius off scale; session-bar axis labels 11.5px; line numbers in the diff 4.2:1; h2 to h4 in the local-edit dialog; "Take upstream" says "the path is shown once it is done" instead of the exact backup path; a file removed upstream shows only "No text preview" and not "Take upstream deletes this file"; "Update all" list rows for versionless updates have an empty right cell; link picker allows A needs B when B already needs A; "Worst first" sits on the status row, detached from Sort; unused health segment is hard to see in dark; at 960 the Library health and Sessions panels stack and push the first card below the fold.
