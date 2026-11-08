# Composition Brief: Harneo, Harnmap and Harnfit

## Output
- Composition: `composition/` (Hyperframes 0.9.9 + GSAP, same structure as `../01-launch/`)
- Video: `brag.mp4`, 1920x1080, 30 fps, 26 s; poster `brag.jpg` (21.2 s, the red Conflicts card) baked as frame 0
- Rebuild: `./build.sh` (Node 22+, Python 3, ffmpeg, a Chrome)

## Source material
- Pitch, angle and storyboard: `brag-plan.md` (the creative contract)
- Captures, `composition/assets/shots/` (2880x1800, 2x the 1440x900 window), from the 0.3.0 release build on a scratch copy of the library:
  - `harnmap-before.png`: Harnmap of SoftwareFactory, side panel open, nothing selected
  - `harnmap-node.png`: the same view with `review` selected (Claude's reading, next steps 1-2-3)
  - `fit-fits.png`, `fit-changes.png`, `fit-conflicts.png`: saved Harnfit verdicts for `handoff`, `pr`, `code-review`
- Copy that must appear verbatim: "Your Claude Code harness, fitted to every project." (welcome screen); the verdict labels "Fits", "Fits with changes", "Conflicts" (app); "Free and open source · MIT", `github.com/ibrahimkobeissy/harneo`

## Scenes (`composition/compositions/`)
| Scene | Film time | File |
|---|---|---|
| Hook | 0.0–3.5 | `hook.html` |
| Before | 3.5–6.0 | `before.html` |
| Reveal | 6.0–9.0 | `reveal.html` (from 01-launch) |
| Harnmap | 9.0–14.5 | `harnmap.html` |
| Harnfit | 14.5–22.0 | `harnfit.html` |
| Outro | 22.0–26.0 | `outro.html` (from 01-launch) |

## Audio
`music/compose.py` writes `assets/music/harneo-theme.wav` (120 BPM, beats every 0.5 s), its cues and per-frame RMS (drives the logo glows). Cues: 6.0 kick and key change, 12.0 node click, 19.0–20.0 dip under the red verdict, 22.0 logo. SFX are Kenney CC0 (`assets/sfx/CREDITS.md`).
