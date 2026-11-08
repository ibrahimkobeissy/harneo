"""Original soundtrack for the Harneo Harnmap / Harnfit brag video. Pure Python, no dependencies.

    python3 music/compose.py OUT.wav [CUES.json] [RMS.js]

120 BPM, 26 s, built on the video's beat grid (beat = 0.5 s, bar = 2 s):
  0-6 s   Bm7 / Gmaj7 pad + pluck only (the noise, the hook and the before)
  6 s     kick enters and the key turns to D major (logo reveal)
  19-20 s drums and pluck drop out under the red Conflicts verdict; 20 s back in
  22 s    resolve on Dmaj9 (logo), fade 25-26 s
Original work made for this repo; covered by the repo's MIT license, no attribution needed.
"""
import array, json, math, random, struct, sys, wave

SR = 44100
BPM = 120
BEAT = 60 / BPM
DUR = 26.0
N = int(SR * (DUR + 0.5))
random.seed(11)

L = array.array("d", bytes(8 * N))
R = array.array("d", bytes(8 * N))


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def add(buf, start, samples, gain, pan=0.0):
    """Mix mono samples into the stereo bus at time `start` (s)."""
    i0 = int(start * SR)
    gl = gain * math.cos((pan + 1) * math.pi / 4)
    gr = gain * math.sin((pan + 1) * math.pi / 4)
    for k, s in enumerate(samples):
        i = i0 + k
        if i >= N:
            break
        L[i] += s * gl
        R[i] += s * gr


# ---------- instruments ----------
TABLE = 2048
WT = array.array("d", [0.0] * TABLE)
for h in range(1, 9):  # soft, band-limited saw
    a = (1 / h) * (0.82 ** (h - 1))
    for k in range(TABLE):
        WT[k] += a * math.sin(2 * math.pi * h * k / TABLE)
peak = max(abs(x) for x in WT)
for k in range(TABLE):
    WT[k] /= peak


def pad_note(freq, length, attack=0.35, release=0.8):
    n = int((length + release) * SR)
    out = array.array("d", [0.0] * n)
    for det in (-0.07, 0.07):
        f = freq * 2 ** (det / 12)
        ph = random.random() * TABLE
        step = f * TABLE / SR
        for i in range(n):
            out[i] += WT[int(ph) % TABLE]
            ph += step
    lp = 0.0
    for i in range(n):  # envelope + one-pole lowpass
        t = i / SR
        env = min(1.0, t / attack) if t < length else max(0.0, 1 - (t - length) / release)
        lp += 0.18 * (out[i] - lp)
        out[i] = lp * env * 0.5
    return out


def pluck(freq, length=1.2, bright=0.5):
    n = int(length * SR)
    p = max(2, int(SR / freq))
    buf = [random.uniform(-1, 1) for _ in range(p)]
    lp = 0.0
    for k in range(p):  # soften the excitation
        lp += bright * (buf[k] - lp)
        buf[k] = lp
    out = array.array("d", [0.0] * n)
    idx = 0
    for i in range(n):
        a = buf[idx]
        b = buf[(idx + 1) % p]
        buf[idx] = 0.4985 * (a + b)
        out[i] = a
        idx = (idx + 1) % p
    for i in range(min(n, 220)):
        out[i] *= i / 220
    return out


def kick(gain=1.0):
    n = int(0.35 * SR)
    out = array.array("d", [0.0] * n)
    ph = 0.0
    for i in range(n):
        t = i / SR
        f = 48 + 70 * math.exp(-t * 28)
        ph += 2 * math.pi * f / SR
        out[i] = math.sin(ph) * math.exp(-t * 9) * gain
    return out


def noise_hit(length, decay, hp=0.85, lpk=0.5):
    n = int(length * SR)
    out = array.array("d", [0.0] * n)
    prev = lp = 0.0
    for i in range(n):
        x = random.uniform(-1, 1)
        h = x - prev * hp  # crude high-pass
        prev = x
        lp += lpk * (h - lp)
        out[i] = lp * math.exp(-i / SR * decay)
    return out


def sub(freq, length):
    n = int(length * SR)
    out = array.array("d", [0.0] * n)
    for i in range(n):
        t = i / SR
        env = min(1, t / 0.02) * math.exp(-t * 2.2)
        out[i] = math.sin(2 * math.pi * freq * t) * env
    return out


# ---------- score ----------
# D major: Dmaj7 | Bm7 | Gmaj7 | A(sus4->maj), roots in octave 2
PROG = [
    (50, [62, 66, 69, 73]),  # Dmaj7
    (47, [62, 66, 69, 71]),  # Bm7
    (43, [62, 67, 71, 74]),  # Gmaj7
    (45, [62, 64, 69, 73]),  # A6sus-ish
]
BARS = 13
HOOK = [(47, [62, 66, 69, 71]), (43, [62, 67, 71, 74]), (47, [62, 66, 69, 74])]  # Bm7, Gmaj7, Bm7
KICK_IN = 6.0
DIP = (19.0, 20.0)       # under the red Conflicts verdict
LOGO = 22.0


def in_dip(t):
    return DIP[0] <= t < DIP[1]


for bar in range(BARS):
    t0 = bar * 2.0
    root, chord = HOOK[bar] if bar < len(HOOK) else PROG[(bar - len(HOOK)) % 4]
    if t0 >= LOGO - 0.01:
        root, chord = 50, [62, 66, 69, 73, 76]  # resolve: Dmaj9
    length = 2.0 if t0 < LOGO else DUR - t0
    for j, m in enumerate(chord):
        add(L, t0, pad_note(hz(m - 12), length), 0.075, pan=-0.5 + j * 0.33)

    # plucked arpeggio, 8ths, up-down over chord tones an octave up
    arp = [chord[0] + 12, chord[1] + 12, chord[2] + 12, chord[3] + 12, chord[2] + 12, chord[1] + 12, chord[3] + 12, chord[2] + 24]
    if t0 < LOGO:
        for s in range(8):
            t = t0 + s * BEAT / 2
            if in_dip(t):
                continue
            g = 0.22 if s % 2 == 0 else 0.15
            add(L, t, pluck(hz(arp[s]), 1.0, 0.45), g, pan=0.35 if s % 2 else -0.25)

    if t0 >= 2.0 and t0 < LOGO:
        # sub bass on beats 1 and 3
        for b in (0, 2):
            t = t0 + b * BEAT
            if not in_dip(t):
                add(L, t, sub(hz(root), 0.9), 0.30)
        # kick on every beat, hat on offbeats, soft snap on 2 and 4
        for b in range(4):
            t = t0 + b * BEAT
            if in_dip(t) or t < KICK_IN:
                continue
            add(L, t, kick(), 0.30)
            if t >= KICK_IN + 2.0:
                add(L, t + BEAT / 2, noise_hit(0.05, 70, 0.95, 0.6), 0.035, pan=0.3)
            if t >= KICK_IN + 3.0 and b in (1, 3):
                add(L, t, noise_hit(0.18, 22, 0.7, 0.35), 0.06, pan=-0.1)

# the kick that opens the reveal, the re-entry after the dip, the logo landing
add(L, KICK_IN, kick(1.2), 0.5)
add(L, DIP[1], kick(1.1), 0.45)
add(L, LOGO, kick(1.2), 0.5)
add(L, LOGO, sub(hz(38), 2.5), 0.32)
for j, m in enumerate([74, 78, 81, 85, 88]):  # strummed Dmaj9
    add(L, LOGO + j * 0.035, pluck(hz(m), 2.6, 0.5), 0.14, pan=-0.4 + j * 0.2)

# ---------- ping-pong delay on the whole bus (3/8 beat) ----------
d = int(SR * BEAT * 0.75)
fb, wet = 0.32, 0.18
dl = array.array("d", [0.0] * N)
dr = array.array("d", [0.0] * N)
for i in range(d, N):
    dl[i] = (R[i - d] + dr[i - d] * fb) * 0.5
    dr[i] = (L[i - d] + dl[i - d] * fb) * 0.5
for i in range(N):
    L[i] += dl[i] * wet
    R[i] += dr[i] * wet

# ---------- fade, soft clip, normalize, write ----------
for i in range(N):
    t = i / SR
    g = 1.0
    if t < 0.08:
        g = t / 0.08
    if t > DUR - 1.0:
        g = max(0.0, 1 - (t - (DUR - 1.0)) / 1.0)
    L[i] = math.tanh(L[i] * g * 1.2)
    R[i] = math.tanh(R[i] * g * 1.2)
peak = max(max(abs(x) for x in L), max(abs(x) for x in R)) or 1
norm = 0.89 / peak
out = sys.argv[1] if len(sys.argv) > 1 else "harneo-theme.wav"
n_out = int(SR * DUR)
with wave.open(out, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    frames = bytearray()
    for i in range(n_out):
        frames += struct.pack("<hh", int(L[i] * norm * 32767), int(R[i] * norm * 32767))
    w.writeframes(bytes(frames))

if len(sys.argv) > 2:
    beats = [round(k * BEAT, 3) for k in range(int(DUR / BEAT))]
    with open(sys.argv[2], "w") as f:
        json.dump({"track": out.split("/")[-1], "bpm": BPM, "duration": DUR, "beats": beats,
                   "strongCues": [KICK_IN, 12.0, DIP[1], LOGO],
                   "sections": {"hook": [0, KICK_IN], "dip": list(DIP), "logo": LOGO, "fadeOut": [DUR - 1.0, DUR]}}, f, indent=1)

if len(sys.argv) > 3:
    # per-frame RMS at 30 fps, normalized, fast attack / slow release: drives the logo glow
    hop = SR // 30
    vals = []
    for fr in range(n_out // hop):
        s = 0.0
        for i in range(fr * hop, fr * hop + hop):
            a = (L[i] + R[i]) / 2
            s += a * a
        vals.append(math.sqrt(s / hop))
    top = max(vals) or 1
    sm, prev = [], 0.0
    for v in vals:
        v /= top
        prev = v if v > prev else prev * 0.85 + v * 0.15
        sm.append(round(prev, 3))
    with open(sys.argv[3], "w") as f:
        f.write("window.HARNEO_RMS=" + json.dumps(sm) + ";\n")
print("wrote", out)
