import type { Point, Rect } from "./common";

/** Between the legs of loops at one stage: wider than an arrowhead (14 px) so they never overlap. */
const LEG_STEP = 18;

export const BADGE_H = 32;
export const BADGE_MIN_W = 56;
export const BADGE_MAX_W = 160;
/**
 * Estimated width per character of the badge name (9 px mono with letter spacing): measured at
 * about 6.5 px in the app; 7 leaves room so a name that fits is never cut by a pixel.
 */
const BADGE_CHAR_W = 7;
const BADGE_PAD = 20;
/** Clear space between the badge and the first valve. */
export const BADGE_GAP = 10;

/** The badge's width: it grows to fit the profile name, up to a maximum (then the name ellipsises). */
export function badgeWidth(name: string): number {
  const w = Math.ceil(name.length * BADGE_CHAR_W + BADGE_PAD);
  return Math.min(BADGE_MAX_W, Math.max(BADGE_MIN_W, w));
}

/** The leg x offsets of each loop end at a stage: loops on the same stage get side-by-side legs. */
export function legOffsets(stageOf: string[]): number[] {
  const total = new Map<string, number>();
  for (const s of stageOf) total.set(s, (total.get(s) ?? 0) + 1);
  const seen = new Map<string, number>();
  return stageOf.map((s) => {
    const k = seen.get(s) ?? 0;
    seen.set(s, k + 1);
    return (k - ((total.get(s) ?? 1) - 1) / 2) * LEG_STEP;
  });
}

/** Space between a loop's arrowhead and the frame of the stage it returns to (design: 6 px). */
export const LOOP_ARROW_GAP = 6;

/** Straight down from the start, across at `runY`, straight up into the end (as the design draws it). */
export function orthogonalPoints(
  start: Point,
  end: Point,
  runY: number,
): Point[] {
  return [start, { x: start.x, y: runY }, { x: end.x, y: runY }, end];
}

/** A polyline as dense points (every few pixels), so label boxes cannot slip between two of them. */
export function densify(points: Point[], step = 4): Point[] {
  const out: Point[] = [];
  points.forEach((p, i) => {
    const prev = points[i - 1];
    if (!prev) {
      out.push(p);
      return;
    }
    const n = Math.max(
      1,
      Math.ceil(Math.hypot(p.x - prev.x, p.y - prev.y) / step),
    );
    for (let k = 1; k <= n; k++) {
      out.push({
        x: prev.x + ((p.x - prev.x) * k) / n,
        y: prev.y + ((p.y - prev.y) * k) / n,
      });
    }
  });
  return out;
}

/**
 * How far to shift the whole map right so the badge (at `left`, `width` wide) clears every line point
 * in its own row by `BADGE_GAP`.
 */
export function badgeShift(beam: Point[], badge: Rect): number {
  const near = beam.filter(
    (p) => p.y + 8 > badge.y && p.y - 8 < badge.y + badge.h,
  );
  if (near.length === 0) return 0;
  const first = Math.min(...near.map((p) => p.x));
  return Math.max(0, badge.x + badge.w + BADGE_GAP + 6 - first);
}
