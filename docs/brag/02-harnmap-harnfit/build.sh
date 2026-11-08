#!/bin/sh
# Rebuild the Harneo Harnmap / Harnfit brag video from source: docs/brag/02-harnmap-harnfit/build.sh
#   1. npm ci in composition/ (Hyperframes + GSAP; postinstall vendors gsap.min.js)
#   2. compose the original soundtrack, its beat cues and per-frame RMS (music/compose.py)
#   3. hyperframes check (the gate), then render brag.mp4
#   4. pull the poster (brag.jpg, the clash and its quoted lines at 26.5 s) and bake it as frame 0
# Needs Node 22+, Python 3, ffmpeg and a Chrome. Telemetry is off.
# Set HYPERFRAMES_BROWSER_PATH to a chrome-headless-shell if system Chrome won't start headless.
set -eu
cd "$(dirname "$0")"
export DO_NOT_TRACK=1 HYPERFRAMES_NO_TELEMETRY=1 HYPERFRAMES_SKIP_SKILLS=1

(cd composition && npm ci)

mkdir -p composition/assets/music
python3 music/compose.py \
  composition/assets/music/harneo-theme.wav \
  composition/assets/music/harneo-theme.cues.json \
  composition/assets/music/harneo-theme.rms.js

(cd composition && npx hyperframes check && npx hyperframes render --quality delivery -o ../brag.mp4)

ffmpeg -v error -y -ss 26.5 -i brag.mp4 -frames:v 1 -q:v 2 brag.jpg
ffmpeg -v error -y -i brag.mp4 -i brag.jpg \
  -filter_complex "[0:v][1:v]overlay=0:0:enable='eq(n,0)'[v]" \
  -map "[v]" -map "0:a?" -c:v libx264 -crf 18 -preset slow -pix_fmt yuv420p \
  -c:a copy -movflags +faststart brag.poster.mp4
mv brag.poster.mp4 brag.mp4
echo "built $(pwd)/brag.mp4 and brag.jpg"
