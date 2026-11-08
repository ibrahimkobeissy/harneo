# Brag Plan: Harneo, Harnmap and Harnfit

## The pitch

**The problem.** The Claude Code ecosystem never sits still. Every day brings a new skill, a new plugin, a new "software factory" someone swears by. Nobody can try them all, and trying even one is a gamble: you copy it into a project, and only after it is installed do you find out that it does the same job as something you already have, or quietly works against the way you work. So the harness you depend on is either frozen, because changing it is risky, or a pile of copies that drift apart from project to project.

**The answer.** Harneo is the workshop for your harness.

- **One library.** Every skill, command, subagent, setting and MCP server you trust lives in one place, audited on import and versioned from its source.
- **Profiles are your factories.** Compose a profile once: a software factory, a research workflow, a writing line. Scaffold new projects from it, refit existing ones, and promote what a project improved back into the library. One source of truth, every project in step.
- **Harnmap: see your factory.** Claude reads the profile and draws it as a production line: which artifact sits at which stage, what is always on, what you reach for at any time, and what naturally comes next. Edit it like a diagram; your changes stay.
- **Harnfit: try it before you add it.** Found something new online? Fit it into your factory's map before it touches a project. Claude says where it would go and gives a verdict: it **fits**, it **fits with changes** (and which changes), or it **conflicts**, quoting the exact lines that clash with what you already run.

You could ask for some of this in a prompt. What you couldn't do before is keep it: one place that holds your factories, draws them, tests every newcomer against them, and keeps every project in step with the result.

**One line:** Harneo maps your Claude Code factory and tells you whether a new skill fits it, before you install anything.

## The angle

The ecosystem is noise; your factory is signal. The video opens on the noise (a stream of skills and plugins, three review tools stacked in one `.claude/` folder), then Harneo turns it into a drawn factory and a calm verdict for each newcomer. The climax is the one red verdict: a popular skill that would have collided with three tools already on the line, with Claude quoting the clash.

## Hook (first 3 seconds)
Real artifact names stream past (texture, not read), and two lines land over them: "A new skill every day." / "A new software factory every week." Every Claude Code user has felt this.

## Key moments
- **Before.** A `.claude/` tree builds itself: `commands/review.md`, `skills/code-review-and-quality/`, `agents/code-reviewer.md`, then the new `skills/code-review/` drops in and the four rows flag red together. "You find out after you install it."
- **Harnmap.** The real map of the SoftwareFactory profile: 41 artifacts, seven stages (Idea → Ship), toolbelt and floor, loops back to Build. A click on `review` opens Claude's reasoning and the three natural next steps, numbered on the map.
- **Harnfit.** Three real verdicts, one after another: `handoff` **Fits** (toolbelt), `pr` **Fits with changes** (Review), `code-review` **Conflicts** (Review), the camera pushing into the red card and its quoted lines.

## Outro
Logo, "Harneo", "Map your factory. Fit what's new.", "Free and open source · MIT", `github.com/ibrahimkobeissy/harneo`.

## Tone
- Preset: polished, same family as the first video (`../01-launch/`) so the two read as a series.
- Interpretation: a tense, busy first six seconds (minor chords, no drums, dense texture), then the kick and the major key arrive with the logo, and the real UI does the talking. One red moment, played straight.

## Format: landscape 1920x1080, 30 fps. Duration: 26 s
Planned at 24 s; two seconds added so the before scene and the three verdicts each clear their reading time.

## Visual identity (DESIGN.md)
Ground `#f8fafc`, ink `#0f172a`, muted `#475569`, accent `#2563eb`; verdict colours from the app: green `#15803d` on `#dcfce7`, amber `#b45309` on `#fef3c7`, red `#b91c1c` on `#fee2e2`. Inter 700 for display, JetBrains Mono for paths.

## Source material and data
Captures of the 0.3.0 release build, run with `HARNEO_DATA_DIR` on a scratch copy of the maintainer's library: public packs only on screen (`addyosmani/agent-skills`, `mattpocock/skills`). The Harnfit verdicts are the ones Claude actually returned (saved fits, Opus). For the captures the scratch copy's profile was set back to version 1, the version the fits and the map were made against, so no "out of date" banner shows. No private source URL, personal path or secret appears.

## Audio
Original track (`music/compose.py`, pure Python, MIT): 120 BPM, bar = 2 s. 0–6 s Bm7 / Gmaj7 pad and pluck only (the noise); 6 s kick and D major arrive with the logo; 19–20 s drums drop under the red verdict; 20 s back in; 22 s Dmaj9 resolve on the logo; fade 25–26 s. CC0 effects (Kenney): key ticks as the tree types, card slides as windows arrive, clicks on the node and on each verdict cut, soft impacts on both logos.

## Storyboard

| # | Scene | Time | On screen | Capture |
|---|---|---|---|---|
| 1 | Hook | 0.0–3.5 | Stream of real artifact names; "A new skill every day." then "A new software factory every week." | none |
| 2 | Before | 3.5–6.0 | `.claude/` tree types itself; four review rows flag red; "You find out after you install it." | none (real names from the conflict verdict) |
| 3 | Reveal | 6.0–9.0 | Logo lands on the kick; "Your Claude Code harness, fitted to every project." (welcome screen) | logo |
| 4 | Harnmap | 9.0–14.5 | "01 · Harnmap"; window rises with the map; "See your factory before you build it."; cursor clicks `review` at 12.0; Claude's reading and next steps 1-2-3; "Every step placed, and what comes next." | `harnmap-before.png`, `harnmap-node.png` |
| 5 | Harnfit | 14.5–22.0 | "02 · Harnfit", "Try it before you add it."; cuts on the beat: `handoff` Fits (14.5), `pr` Fits with changes (16.5), `code-review` Conflicts (18.5); push into the red card; "Claude quotes the lines that clash." | `fit-fits.png`, `fit-changes.png`, `fit-conflicts.png` |
| 6 | Outro | 22.0–26.0 | Logo, "Harneo", "Map your factory. Fit what's new.", MIT pill, URL | logo |

## Share copy
See `share-copy.txt`.
