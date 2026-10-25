---
target: "library screens (#49-#54)"
total_score: 27
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 2
target_identity: "file:src/features/library"
timestamp: 2026-10-22T20-20-09Z
slug: src-features-library
---
Method: dual-agent (A: design review · B: detector + browser overlay). Tauri/WebKitGTK exposes no CDP, so both rendered the built frontend in headless Chromium with stubbed IPC.

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|---|---|---|
| 1 | Visibility of System Status | 3 | No confirmation after tag/collection changes; filtered count says "3 artifacts", not "3 of 61" |
| 2 | Match System / Real World | 3 | "bundle" undefined on the welcome |
| 3 | User Control and Freedom | 3 | No undo for one-click untag / remove-from-collection; New collection modal survives route change |
| 4 | Consistency and Standards | 2 | Two "Remove" buttons with different meanings; warning fills neutral vs amber badges; untinted review tags |
| 5 | Error Prevention | 3 | Artifacts flagged "1 error" can be ticked without reading the error |
| 6 | Recognition Rather Than Recall | 3 | Active filters not summarised by the results; truncated labels have no full text |
| 7 | Flexibility and Efficiency | 2 | Only Ctrl K / ? / Esc; no `/`, no grid arrow keys, no bulk tag/collect |
| 8 | Aesthetic and Minimalist Design | 3 | Kind label repeats the name; permanent note on collection screens |
| 9 | Error Recovery | 3 | Import errors excellent; elsewhere raw core error strings |
| 10 | Help and Documentation | 2 | Thin shortcuts list; diagnostic codes unexplained |
| **Total** | | **27/40** | **Acceptable (top of band)** |

## Design Specificity Verdict

LLM: grounded in the detail panel and import flow (provenance with commit, stable diagnostic codes, `${KEY}` callout, already-in-library rows); the Library card itself is generic now that trust/usage are out of scope — its footer proves little. Category-standard look executed faithfully, both themes first-class.

Deterministic scan: CLI `impeccable detect` on src/features/library, src/features/import, Sidebar, Palette: 0 findings (regex rules only). Browser overlay (Chromium, 5 views × 2 themes): overused-font (Inter, false positive: DESIGN.md's chosen face) plus 4 real contrast misses: white on `--blue` in dark (3.7:1, primary buttons + active stepper), `--blue` on `--blue-soft` in dark (4.3:1, active pager + selected rule chips), `--t3` on `--sunk` in light (4.3:1, file path in diagnostics), and no `::placeholder` colour anywhere (browser default, 3.7:1 dark in Chromium). The LLM review missed the contrast tokens; the detector cannot see the IA issues below.

## Priority Issues

1. [P1] Library results have no active-filter summary and no primary action — prototype's results toolbar and Import button dropped (index.tsx:240-300, FilterColumn.tsx:89-102). Fix: chips per active filter with ×, "n of N", Clear all, Save as smart collection; Import as the blue primary in the title row. /impeccable layout
2. [P1] "Remove from collection" next to danger "Remove" (DetailPanel.tsx:353-372). Fix: danger becomes "Delete from library…" or moves to an overflow menu. /impeccable clarify
3. [P2] Dark-mode contrast tokens and missing placeholder colour (tokens.css:146-148/211-213, Button.module.css:24, Library.module.css:499, Collections.module.css:203, Detail.module.css:178). Fix: darker `--blue` fill for white text in dark, lighter blue text on blue-soft, `--t2` for paths on sunk, global `::placeholder { color: var(--t3) }`. /impeccable audit
4. [P2] ≤1180px behaviour from DESIGN.md missing: filter column stays at 960, source lines truncate to "addyos…", kind cards stay two-up, rail collections all the same icon. Fix: Filters button + sheet at 75rem, stack cards at 62.5rem, protect the source line, initials in the rail. /impeccable adapt
5. [P2] Import Choose step at scale: diagnostic badges can't be opened, Continue below the fold, no global select all/none (ChooseStep.tsx). Fix: expandable diagnostics, global select + "only without errors", sticky footer. Plus craft slips at trust moments: bullets in the secrets list (ReviewStep.tsx:71), Add tag menu overflows 4px (TagsSection.module.css:59-65), warning diagnostics on neutral fill. /impeccable harden, /impeccable polish

## Persona Red Flags

Alex: three shortcuts only; no `/`, no j/k; no bulk tagging; can't add tags in import review; Smart re-chosen after "Save as smart collection".
Sam: list rows are `<tr tabIndex=0 onClick>` with no role (ItemTable.tsx:57-64); `<span>` inside `<ol>` in Stepper.tsx:13-14; silent tag/collection changes (no live region); truncated labels without full text.
Jordan: "bundle" undefined; Smart/Manual badges assume the dialog was read; icon-only rail at 960; "Nothing found" styled as a red error; diagnostic codes unexplained; nothing marks newly imported cards.

## Minor Observations

Kind label duplicates name (ItemCard.tsx:24-26); US-locale dates (ItemMeta.tsx:50); import back link under the title; permanent collection note (CollectionBar.tsx:249-252); `#testing` vs `testing`; "Untitled collection" silently used; untinted review tags; palette "No matches" doesn't point to Library search.

## Questions to Consider

- Until trust and usage arrive, what does a card prove? Files, references, diagnostics and commit SHA instead of an import date?
- Why does the screen where every journey starts have no blue primary action?
- Is a smart collection just saved Library URL state — could "Save as smart collection" be an inline name field instead of a modal?
