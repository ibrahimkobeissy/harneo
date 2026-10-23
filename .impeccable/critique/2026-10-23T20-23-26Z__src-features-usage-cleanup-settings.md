---
target: "Cleanup screen + prune/remove dialogs, library usage UI (Insights, UsageSection, UsageBars, card footers, Sessions/Status, Status filter), Settings screen (#117)"
total_score: 27
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:/home/ibra/projects/harneo/src/features/cleanup"
timestamp: 2026-10-23T20-23-26Z
slug: src-features-usage-cleanup-settings
---
⚠️ DEGRADED: single-context (the run was itself a sub-agent briefed to do the work directly, so A and B ran back to back in one context: A from screenshots, prototype, specs and code first, B from the detector and measurements second). Tauri/WebKitGTK exposes no CDP; the built frontend was rendered in headless Chromium with stubbed IPC, light and dark at 1440 and 960 (72 screenshots: cleanup full/empty/error, prune/remove/update-all dialogs, library cards/list/filtered/first-index, detail with and without Usage, Settings found/missing/indexing/invalid), and the detector ran through `--cdp` against it (4 routes x 2 themes x 2 widths).

## Design Health Score: 27/40 (Acceptable)

| # | Heuristic | Score | Key issue |
|---|---|---|---|
| 1 | System status | 3 | Index progress, "Reading logs: N of M", "Never checked" are shown in Library and Settings; Cleanup knows nothing of the index, and Settings autosave confirms only to screen readers |
| 2 | Real world | 3 | "Remove from profiles", "Nothing was changed", "unused 2 months ago" are plain; "Unused for 8+ weeks, in every project" claims a per-project measure the logs do not give; "Found" is a status used as a row title |
| 3 | Control | 3 | Cancel is focused first, Esc works; Esc and the scrim also close the prune dialog while it is switching projects; no way back after a prune except by hand |
| 4 | Consistency | 3 | Same group chrome in Cleanup and Settings; but Cleanup prints `2026-07-10` where the library says "2 months ago", the detail panel has two "Used in", Sessions numbers sit left-aligned in the table, Settings is 720 px wide and Cleanup 1040 |
| 5 | Error prevention | 2 | Before the first index every old artifact computes as unused and Cleanup lists it with a Remove button; a security finding that sits in profiles offers only a delete with "remove it from them first" and no action; the unused window saves on every keystroke |
| 6 | Recognition | 3 | Status filter, legend buttons and "Review 5 issues" connect Library to Cleanup; Cleanup rows carry no kind icon and the artifact name does not look like a link |
| 7 | Efficiency | 2 | 12 unused rows, one dialog per row, no select-several, no "remove all never-used and in no profile"; Sessions column cannot be scanned (left-aligned); no keyboard route beyond tabbing |
| 8 | Minimalism | 3 | Calm, one idea per group; Updates and Live rescan are two panels holding one row each, Data folder is stated twice (Data and About), the import-detection paragraph is a six-line wall |
| 9 | Error recovery | 2 | Cleanup load error is raw `String(err)` ("database is locked") with no Retry; an unfound CLI says "Install Claude Code (https://code.claude.com)" as dead text; update-all gives a title that says 3 and a button that says 2 |
| 10 | Help | 3 | Settings rows explain what is read, sent and never written (the bundles.md Decision 2 off-by-default disclosure is there and honest); Cleanup never says how "unused" is computed or when the logs were last read |
| **Total** | | **27/40** | **Acceptable** |

## Design Specificity

**LLM assessment (before the detector).** Cleanup and the library insights are recognisably Harneo: the "Review 5 issues" hand-off from the health bar, the "Remove from profiles ... switches 2 projects, with a backup" confirm, and the explicit "Not tracked: Claude's logs don't record when settings are used" are product thinking, not template. The prune dialog is the best screen in scope: it lists exactly which profile versions change, which projects switch and which stay pinned. What is generic is the row furniture: Cleanup rows are bare name-plus-kind-text lines with the type icons the prototype used (`projects.js:111-150`) removed, so a list of 14 "old-helper-N" rows reads as a spreadsheet; the Sessions chart has eight unlabeled pale bars and tooltips only; and Settings is a stack of four identical panels where two hold a single switch. Voice is right (terse, quantified). Visual character is the established Vercel/Linear register, executed cleanly.

**Deterministic scan.** CLI scan of `src/features/cleanup`, `src/features/settings`, `Insights.tsx`, `UsageSection.tsx`, `UsageBars.tsx`: 0 findings (exit clean). CDP scan of the rendered routes: `overused-font` (Inter, every route) is a false positive for this product: DESIGN.md commits to Inter deliberately. `line-length` (Settings, ~106 characters on 3 of 4 lines of the "Detect bundles and tags at import" paragraph, both widths and themes) is real and the same paragraph is the densest text in scope. No contrast findings; manual measurements follow in the issues. Detector missed everything behavioural (index-blind Cleanup, destructive dead end, mid-apply Esc, duplicate "Used in").

**Visual overlays.** Not available: the Tauri app has no live-server tab here and the scan ran headless; no overlay was injected.

## Overall Impression

The flows are safe and the prune dialog is exemplary, but Cleanup trusts a number it should qualify: it presents "unused" without knowing whether the usage index has ever run, and it offers a delete as the only action on a risky artifact that profiles still install. The library insights are one polish pass away; Settings is correct but padded with chrome.

## What's Working

- **Prune confirm**: lists profile version bumps (`Heavy v3 -> v4`), the projects that switch, and the pinned ones that stay, with the primary button saying "Remove and push to 2 projects". Precise and quantified, exactly the brand voice.
- **Library to Cleanup hand-off**: the health legend buttons filter the grid with `aria-pressed`, "Review 5 issues" jumps to Cleanup, and "See all 14 in the library" jumps back with `status=unused`. Before the first index the library refuses to show a false "Unused" (it counts those as current and hides the filter).
- **Honest Settings copy**: "Session logs (read only)... never written", the explicit data disclosure for detection at import, "Off by default", and "Needs the Claude CLI" on the disabled controls. Switches are real `role=switch` buttons with labels and visible focus.

## Priority Issues

**[P1] Cleanup is blind to the usage index** (`src/features/cleanup/index.tsx:289-327`, `Cleanup.module.css`)
- Why: `Insights.tsx:35-48` and `ItemCard` treat every tracked artifact as unused until an index run completes and hide that status; Cleanup does not use `useUsageIndex` at all. On a first launch, or when `~/.claude/projects/` cannot be read, the Unused group lists everything imported more than 8 weeks ago as "never used" with "Remove from profiles" and "Remove from library" buttons; with nothing found it says "Nothing to clean up" instead of "not read yet". Deleting from a wrong list is the dangerous moment the product promises to prevent.
- Fix: read `indexed`, `progress` and `error` from `useUsageIndex`. Until indexed, render the Unused group as a status line ("Reading Claude's session logs: 12 of 40 files. Unused artifacts appear when this finishes.") with no rows; on index error say so with Retry (`refresh`); in the empty state add "Usage last read <time>". Reload on `runs`. Test with an un-indexed stub.
- Command: /impeccable harden

**[P1] A risky artifact in profiles has no safe action** (`src/features/cleanup/index.tsx:236-250`, `Dialogs.tsx:68-125`)
- Why: the Security findings row always offers the red "Remove from library". The confirm then says "It is in the profiles below; remove it from them first if they should stop installing it" and lists them, with no way to do it from there, although `PruneDialog` exists for exactly this and the Unused group already switches on `profiles.length`. A user either deletes the library copy (profiles keep referencing a missing artifact) or leaves the dialog and hunts through Profiles.
- Fix: in the findings row use the same split as the unused rows: if `a.profiles.length > 0` the button is "Remove from profiles" (opens `PruneDialog`, whose confirm says the library copy stays); otherwise "Remove from library". Add a "View findings" link to `/security` beside the "4 findings" badge, and drop the dead-end sentence in `RemoveFromLibrary`.
- Command: /impeccable harden

**[P1] Prune dialog can be dismissed while it is switching projects** (`src/features/cleanup/Dialogs.tsx:37-45,47-60`)
- Why: `Confirm` closes on Esc and scrim click and keeps Cancel enabled while `busy`. The apply continues (profiles saved, projects switched, backups written), but the user is back on the list, can fire another prune on the same artifact, and the only record is a 6-second toast, which is also the only place the result is reported. The previous critiques (#94, #111) found the same pattern in Install and Drift.
- Fix: when `busy`, ignore Esc and scrim, disable Cancel, and set `aria-busy` on the form; keep "Removing..." as the label. Make the failure toast persistent (it already is for `bad`) and add the profile/project counts to the success toast (already there) plus a "View projects" action.
- Command: /impeccable harden

**[P2] Update-all dialog contradicts itself** (`src/features/cleanup/Dialogs.tsx:258-262`)
- Why: title "Update 3 artifacts?" over a button "Update 2" (one is skipped for local edits) and a warning that says "review it one by one" with no link. Reads as a bug at a confirm.
- Fix: title `Update ${n} of ${total} artifacts?` (or just `Update ${n}`), and turn the names in the warning into `Link`s to `/library?open=<id>` so "review one by one" is one click.
- Command: /impeccable clarify

**[P2] Cleanup rows are harder to scan than their prototype** (`src/features/cleanup/index.tsx:82-93,110-112,291-327`, `Cleanup.module.css:58-80`)
- Why: raw ISO dates ("last used 2026-07-10") where the library says "2 months ago" (`relativeDay` already exists in `library/usage.ts`); no kind icon (the prototype's `typeIc`), so 14 rows differ only by a grey word; the artifact name is a link styled as plain bold text with underline only on hover; the group title "Unused for 8+ weeks, in every project" promises per-project data the logs do not give; a row of 12 identical outline buttons, six of them red, makes the red mean nothing. Cleanup load errors are `String(err)` with no Retry (`index.tsx:213-219`).
- Fix: use `relativeDay` with the ISO in `title`; render `KindChip`/kind icon from `ui` in `Who`; give `.name` the blue link colour or an underline; retitle "Unused for 8+ weeks"; make "Remove from library" a quiet danger (text-red, no outline) until hover, keeping the solid red in the confirm; add a Retry button to the load error.
- Command: /impeccable polish

**[P2] Library usage visuals: low-contrast marks, no numbers, a doubled label** (`Usage.module.css:93-101,56-62`, `UsageBars.tsx`, `UsageSection.tsx:88`, `ItemTable.tsx:126`, `Insights.tsx:35-48`)
- Why: measured non-text contrast: light `--chart` bars `#bfdbfe` on white 1.42:1, light unused segment/dot `--line2` 1.48:1 (dark compensates with an edge, light does not), dark `--chart` bars `#1e3a8a` on `#131c31` 1.64:1; only the newest bar is readable and values are in `title` tooltips and a visually hidden list, so a sighted keyboard user gets no numbers. The detail drawer shows "Used in 2 projects" inside Usage and a second "Used in" heading (profiles) below it. Sessions numbers in the list are left-aligned, so 14, 9, 2 do not line up. While indexing, the health bar says "10 current, 0 unused" (unused counted as current) with no hint. Health panel is stretched to the chart's height, leaving a blank lower third.
- Fix: bars at >=3:1 (light `#93c5fd`, dark `#2563eb`), add the max as a small mono label at the top-left of the chart and the latest value above the last bar; give the light unused segment the same `--seg-edge`; rename the drawer line "Seen in 2 projects" (or "Sessions in 2 projects"); `.sessionsCell { text-align: right }` with matching `th`; while `!indexed` show the legend with "reading logs" instead of numbers for current/unused; `align-content: start` on the health panel.
- Command: /impeccable polish

**[P2] Settings: chrome without content, and one real input bug** (`UsageSettings.tsx:62-66,95-124,138-150`, `ClaudeSettings.tsx:91-94,140-153`, `UpdatesPanel.tsx:50-56`, `LiveRescan.tsx`, `index.tsx:15-23`)
- Why: (1) "Unused after N weeks" saves on every keystroke, so typing 12 saves 1 then 12, Cleanup reads the value once at mount, and the only confirmation is a screen-reader announcement. (2) "Session logs" row packs label, "Last indexed: 10/23/2026, 11:30:00 AM", path and button in one line; the sentence orphans "AM", and while indexing it reads "Last indexed: never Reading logs..." with no punctuation. (3) "Check for updates at start" says "Once a day", two different promises. (4) The CLI hint prints `https://code.claude.com` as dead text although the opener plugin is in the app. (5) Updates and Live rescan are panels with one row each; Data folder appears in Data and again in About. (6) The detection paragraph is ~106 characters per line over six lines (detector `line-length`, measured at both widths). (7) No "copy path" on the paths (the prototype has it). (8) "Found" is the row title.
- Fix: save the number on blur/Enter (keep the validation), show an inline "Saved" for 2 s; split the Session logs row into the sentence, a `Last indexed` line and the status line separately; one wording for updates ("Once a day, when Harneo starts"); make the URL an opener link; fold Updates and Live rescan into one "Background" group, drop the Data-folder row from About (or from Data) and add a copy button; wrap the detection copy to `max-width: 65ch` and put the three facts (what is sent, how much, what is withheld) as a short list; retitle the CLI row "Claude CLI: found" with the path under it.
- Command: /impeccable layout

## Persona Red Flags

**Alex (Power User)**: cleaning 12 unused artifacts means 12 dialogs; no select-several, no "remove everything unused and in no profile" action. No keyboard shortcut or arrow navigation in Cleanup lists. Sessions column left-aligned, so ranking by eye fails. Typing 12 in Settings saves 1 first.

**Jordan (First-Timer)**: "Unused for 8+ weeks, in every project" and "Repeated across profiles" assume profile/project vocabulary; nothing says where the usage numbers come from on Cleanup (only in Settings and the empty chart). On a fresh install Cleanup shows every old artifact as "never used" before the logs were read and offers to delete it. "Found" as a row heading is unclear.

**Sam (Accessibility)**: bars at 1.4-1.6:1 are decorative-only for low vision and their values live in tooltips; the unused segment in light has no edge. The artifact link in Cleanup differs from body text only by hover underline (no non-hover cue). Switches are labelled and focusable (good); inline "Saved" is announced only through `announce`, but the invalid-weeks error is correctly `role=alert`.

**Riley (Stress Tester)**: cleanup list error leaves a blank page with no retry; a failed update-all says "Updated 0 artifacts; 2 failed"; deleting a finding that profiles reference leaves dangling profile entries; closing the prune dialog mid-run allows a second prune on the same artifact; a 52-char Settings path breaks mid-word (`word-break: break-all`).

## Minor Observations

- `Cleanup.module.css` `.toast` text uses `--green`/`--red` on soft fills; fine at 4.5:1, but `Dismiss` is `<button>` styled as an underlined text with no focus style (`.toastClose`).
- The Outdated "Review" link and the Duplicates "Show in library" link are blue text links beside outline buttons: two action styles in one list.
- `Repeated across profiles` rows have no action; the note below them says what to do but offers no "Create base profile" path.
- `lastUsedText` shows "never used" for artifacts imported 9 weeks ago; "not used since import" is truer.
- Settings label `<b>` inside `<label>`: fine for AT, but the description (`.sec`) is not linked with `aria-describedby`, so the privacy copy on the detection switch is not read with it.
- `Switch` has no text state; the on/off state is carried by position and colour alone (position is enough, but "On"/"Off" is cheap).
- Library health legend: "with findings" count and the Status filter's "Findings" use different words.
- `AboutPanel` hijacked into group look by CSS selectors on child markup (`Settings.module.css:122-146`): fragile; give `AboutPanel` a `Group`-compatible variant.

## Questions to Consider

- If unused only means anything once the logs have been read, should Cleanup lead with "Last read 9:30, 3 sessions found" so the number is auditable?
- What would "Clean up everything unused and in no profile" look like as one reviewed dialog instead of N?
- Does the Sessions chart need to be a chart at all, or would one number and a sparkline beside each card do the same job?
- Why does a finding sit in Cleanup as "remove" when the product's whole identity is the safe way to remove things?

## Fix tickets (file-disjoint)

1. **Cleanup screen** (`src/features/cleanup/`): P1 index-blind Unused group + Retry; P1 findings-in-profiles action via PruneDialog and "View findings" link; P1 block Esc/scrim/Cancel while busy; P2 update-all title and linked names; P2 relative dates, kind icon, link-looking names, retitle, quieter danger buttons.
2. **Library usage UI** (`src/features/library/{Insights,UsageSection,UsageBars,Usage.module.css,ItemTable}.tsx`): chart and segment contrast, value labels, "Used in" rename, right-aligned Sessions, health-bar copy while indexing, panel alignment.
3. **Settings** (`src/features/settings/`, `src/app/AboutPanel.tsx`): save-on-blur with inline Saved, Session logs row layout and copy, updates wording, opener link, merge Updates + Live rescan into Background, drop duplicate Data folder, copy-path buttons, 65ch limit and list for the detection disclosure, row title for the CLI.
