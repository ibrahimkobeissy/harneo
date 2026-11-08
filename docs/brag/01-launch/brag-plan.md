# Brag Plan: Harneo

## What is this app?
A free, open-source desktop app (Tauri) that keeps every Claude Code skill, command, subagent, setting and MCP server you trust in one library, composes profiles from it, and installs the right profile into any project: previewed, backed up, reversible.

## The angle
The quiet tool that refuses to lose your files. Every Claude Code user has copied `.claude/` from repo to repo by hand and watched the copies drift. Harneo is the opposite of that copy: it audits what you import, builds the harness once, and shows every write before it happens, down to the one permission rule a refit would have deleted. The video is a calm, Linear-grade product film where the real UI does the talking and the one red warning is the climax.

## Hook (first 2-3 seconds)
A terminal line types itself: `cp -r ../old-repo/.claude .` then it duplicates down the frame, repo after repo, the copies slightly off. The line under it lands: "Copied by hand from repo to repo." (README, "Why"). It reads as every viewer's own habit.

## Key moments (the middle)
- **Audit on import.** The real import screen: `auto-deploy` comes in as **Risky 50**, two **Critical** findings ("Tries to override the agent's instructions", "Downloads and runs code") with the exact offending lines. Harneo held it back on its own.
- **One library, one profile.** The Library grid (19 artifacts, library health bar, sessions-per-week chart), then the profile editor: artifacts click to "Added", the profile becomes **Heavy v1**.
- **The refit that doesn't lose a file.** The compare table: `settings.json: 1 permission rule will be lost`, the rule shown (`"Bash(stripe listen)" (allow)`), a cursor clicks **Keep** and the red row turns calm.

## Outro / punchline
Logo lands. "Harneo. A harness manager for Claude Code." Under it: "Free and open source · MIT" and `github.com/ibrahimkobeissy/harneo`. Holds 2.5 s.

## User flow worth showing
Import a pack (audit catches the risky skill) → compose a profile (Heavy v1) → refit an existing project (compare, warning, Keep, dry-run preview).

## Tone
- Preset: polished
- Creative direction: Linear / Vercel-grade product film, white and slate with one blue accent; the real UI does the talking.
- Interpretation: few words, long confident holds, soft transitions with a cause (cards growing into screens, a click opening the next state). No jokes; the only drama is the red warning, played straight.

## Format: landscape — 1920x1080, 30 fps
## Duration: 24 s (planned 22 s; lengthened so every line clears its reading time)

## Visual identity (from the project, DESIGN.md frontmatter)
- Background: `#f8fafc` (side) / `#ffffff` (bg), cards `#ffffff` on `#f1f5f9` (sunk)
- Accent: `#2563eb` (blue), `#1d4ed8` hover
- Text: `#0f172a` (t1), `#475569` (t2), `#64748b` (t3), hairlines `#e2e8f0`
- Alerts: red `#b91c1c` on `#fee2e2`; green `#15803d` on `#dcfce7`
- Display font: Inter 700, tracking -0.02em
- Body / code: Inter; JetBrains Mono for paths and code
- Strongest visual element: the app itself at 2× (captured from the real release build), especially the red refit warning and the Critical audit cards.

## Share copy (draft)
Harneo keeps every Claude Code skill, subagent, setting and MCP server you trust in one library, audits it on import, and refits any project without losing a line. Free, open source, MIT.

## Audio direction
- Role: warm bed with sparse, motion-matched accents.
- Music: **original track composed for this video** (synthesized in code, 120 BPM, D major: soft pad, plucked arpeggio, sub bass, light kick from bar 2). No third-party license, no attribution needed; ships under the repo's MIT license. (The bundled CC BY tracks were set aside at the user's request.)
- Music treatment: sparse (pad + pluck only) under the hook; kick enters at 3.0 s on the reveal; drop to pad only under the red warning (17.0-18.0 s); the Keep click brings it back at 18.0 s; Dmaj9 resolve on the logo at 20.0 s; fade 23.0-24.0 s.
- Music cue guidance: beat grid every 0.50 s from 0.00 s (bar = 2.00 s). Strong cues: 3.00 s (reveal), 18.00 s (Keep click), 20.00 s (logo). Profile Add clicks on the 0.25 s grid (11.50-13.25 s); audit findings at 9.0 and 9.5 s.
- Audio-reactive treatment: subtle; the blue glow behind the logo breathes with the track's RMS. No waveform visuals.
- SFX posture: sparse, CC0 only (Kenney.nl via /brag): soft key ticks for the typed hook, a card slide as screens arrive, one click on Keep, one soft impact on the logo.
- Audio-coupled moments: typed hook line, screen arrivals, the Keep click, the logo landing.
- Restraint rule: no risers, no whooshes on every cut, nothing louder than the music.

## Fictional data and stand-ins
All on-screen data comes from a throwaway sandbox seeded for this video: packs `team-harness`, `design-kit`, `community-skills` (with a deliberately malicious `auto-deploy` skill), projects `shop-api` and `billing-worker`, and fake session logs. No real user data. Screens that showed the sandbox's long `/tmp/...` path (Projects, Settings, Refit folder step) were left out of the cut, so no real path appears in the video.

## Storyboard

### Scene 1 — Hook — 0.0-3.0 s
White ground. A mono terminal line types `cp -r ../old-repo/.claude .`; it repeats down the frame as faded copies (repo-a, repo-b, repo-c), each one slightly different. "Copied by hand, repo to repo." lands below in Inter 700.
Sequential/interaction: yes — typed line, then 3 copies 0.12 s apart (texture, not read).
Audio intent: intimate, a little tired. Key ticks on the typing.
Audio-coupled idea: per-character key ticks.
Music: pad + pluck only.
Transition mood: soft → Scene 2 (copies blur out, blue logo tile scales in from the cursor).

### Scene 2 — Reveal — 3.0-6.0 s
The Harneo logo tile and wordmark, then the app's own headline: "Your Claude Code harness, fitted to every project." (verbatim from the welcome screen). Holds ~2 s.
Sequential/interaction: logo, then headline words in quick stagger, then hold.
Audio intent: arrival; kick enters.
Audio-coupled idea: beat-locked logo on 2.00 s cue region (lands 2.5 s on a beat).
Transition mood: clean → Scene 3 (headline lifts, the app window rises into frame).

### Scene 3 — Import audit — 6.0-11.0 s
Real Library screen in an app window (19 artifacts, health bar, sessions chart). Caption: "Every artifact audited on import." Then the window dims and the real audit card punches in on top: `auto-deploy` Risky 50, two Critical findings. Sub-caption: "1 artifact held back for critical findings." (verbatim UI).
Sequential/interaction: window arrives; audit card slides up; the two Critical rows reveal one by one (1.0 s apart).
Audio intent: confident, then a small tightening.
Audio-coupled idea: card slide on the audit card arrival.
Transition mood: soft → Scene 4.

### Scene 4 — Profile — 11.0-14.0 s
Real profile editor (Heavy v1: 8 artifacts, 11 files). Caption: "Compose a profile once." A cursor clicks one "Add" which flips to "Added" (real before/after states).
Sequential/interaction: simulated click on Add.
Audio intent: steady groove.
Audio-coupled idea: soft click on Add.
Transition mood: clean slide → Scene 5.

### Scene 5 — Refit — 14.0-20.0 s
Real refit compare screen. Camera pushes into the red banner: "settings.json: 1 permission rule will be lost" and the lost rule `"Bash(stripe listen)" (allow)`. Hold. Cursor clicks **Keep**; the screen settles into the calm after-state. Caption: "Previewed. Backed up. Reversible." (README tagline).
Sequential/interaction: push-in, hold, cursor click, state swap.
Audio intent: music drops to pad under the warning, returns on the click.
Audio-coupled idea: one clean click on Keep.
Transition mood: dramatic but quiet → Scene 6 (white flood from the Keep button).

### Scene 6 — Outro — 20.0-24.0 s
Logo + "Harneo", "A harness manager for Claude Code.", "Free and open source · MIT", `github.com/ibrahimkobeissy/harneo`. Holds to the end.
Sequential/interaction: none beyond the landing.
Audio intent: resolve; soft impact on the logo; fade out.
Transition mood: end.

**Music mood for this video:** warm, minimal, confident (original composition).
**Audio summary:** a sparse plucked loop that gains a kick on the reveal, steps back for the one red warning, and resolves on the logo.
