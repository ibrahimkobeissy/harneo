---
target: "audit screens (#78-#80)"
total_score: 23
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:src/features/security"
timestamp: 2026-10-23T01-02-55Z
slug: src-features-security
---
Method: dual-agent (A: design review · B: detector + browser overlay). Tauri/WebKitGTK exposes no CDP; both rendered the built frontend in headless Chromium with stubbed IPC.

## Design Health Score: 23/40 (Acceptable)

| # | Heuristic | Score | Key issue |
|---|---|---|---|
| 1 | System status | 2 | Reach tiles filter a table ~2,500px below; re-audit "2 changed" doesn't say which |
| 2 | Real world | 2 | Raw rule ids ("Critical finding: rce"); tier "Review" collides with the "Review" step; "10 network hosts" counts artifacts; `*` unexplained |
| 3 | Control | 3 | "Exclude Risky and Dangerous" is one-way |
| 4 | Consistency | 1 | Three trust meters, three tier maps, three severity colour maps |
| 5 | Error prevention | 3 | Restoring a Dangerous artifact is one ghost click with no finding restated |
| 6 | Recognition | 2 | Excluded rows show only the first rule id; cards ignore findingCount |
| 7 | Efficiency | 3 | No jump from a finding row to the finding; no worst-first preset |
| 8 | Minimalism | 2 | Security findings table is a pink wall; red "0 Dangerous" pill |
| 9 | Error recovery | 3 | Not-audited state has no link |
| 10 | Help | 2 | No explanation of tiers, `*`, or rules at the decision point |

## Design Specificity
Evidence-first language is specific to Harneo (mono file:line, redacted evidence, real hosts/commands/env, "nothing was run"). The miss: the trust signal is weakest where users live (Library cards) and the histogram hides the Dangerous buckets.

Detector: CLI 2 side-tab warnings (Detail.module.css:242 finding cards, Import.module.css:932 flagged rows). Browser: Security light — `.sevRow span` 4.3:1, `.where` 3.9:1 on red-soft and 11.5px (tiny-text); overused-font (Inter, false positive). Measured: import `.tierScore` fails AA in light for all four tiers (3.5–4.3:1, `opacity: 0.85` at Import.module.css:818); Risky uses tag token `--tint-orange` on Security (pale in dark, lighter than Review).

## Priority Issues
1. [P1] Dangerous artifacts aren't visibly flagged in the Library (36px sliver + number; no tier word, no finding count; lint badges look like security findings).
2. [P1] Trust colours inconsistent; Risky reads safer than Review in dark; three TrustMeter/tier/severity maps; orange undocumented in DESIGN.md.
3. [P1] Security findings table: critical and high rows both washed red; the Critical badge disappears on the red row.
4. [P2] Reach tiles act off-screen and mislabel counts.
5. [P2] Histogram hides the Dangerous/Risky buckets (linear scale, 4% minimum).

## Persona red flags
Alex: no finding-row → finding jump; no worst-first; no undo for Exclude Risky and Dangerous. Sam: Security trust cells announce only a number; expand label says "diagnostics" for audit findings; Critical badge without boundary. Jordan: rule-id jargon, Review/Review collision, `*` unexplained, not-audited copy without a link, no "N held back" on Review.

## Minor
Uppercase severity in import; side stripes; tile icon shrinks; duplicate "Findings" headings; zero-count tier washes; repeated "network/shell" labels in the Review summary; Security audit summary below the tag list; Excluded rows without findings; Trust filter group off-screen below Source; slider vs tier contradiction; filter sheet and drawer both open at 960 after navigation; re-audit doesn't list changed artifacts.
