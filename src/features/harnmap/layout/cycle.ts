import {
  pick,
  assemble,
  center,
  GAP,
  overlaps,
  type Layout,
  type LayoutInput,
  type Point,
} from "./common";

/**
 * Stages around an ellipse, first at the top and clockwise, each frame hanging down from its point.
 * The ellipse grows by 15 % until no two frames touch, so any number of stations fits.
 */
export function layoutCycle(input: LayoutInput): Layout {
  return assemble(
    input,
    (heights, widths) => ellipse(heights, widths),
    (frames) => {
      const heads = frames.map((f) => center(f.header));
      if (heads.length === 0) return { loopsInside: true };
      const from = pick(heads, heads.length - 1);
      const to = pick(heads, 0);
      const mid = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
      const c = {
        x: heads.reduce((s, p) => s + p.x, 0) / heads.length,
        y: heads.reduce((s, p) => s + p.y, 0) / heads.length,
      };
      return {
        loopsInside: true,
        closedBeam: true,
        returnArrow: {
          from,
          to,
          via: {
            x: mid.x + (mid.x - c.x) * 0.3,
            y: mid.y + (mid.y - c.y) * 0.3,
          },
        },
      };
    },
  );
}

function ellipse(heights: number[], widths: number[]): Point[] {
  const n = heights.length;
  const fw = Math.max(0, ...widths);
  let r = Math.max(120, ((fw + GAP) * n) / (2 * Math.PI));
  for (;;) {
    const pts = heights.map((_, i) => {
      const a = -Math.PI / 2 + (2 * Math.PI * i) / n;
      return {
        x: Math.round(r * Math.cos(a) - pick(widths, i) / 2),
        y: Math.round(0.7 * r * Math.sin(a)),
      };
    });
    const rects = pts.map((p, i) => ({
      x: p.x - GAP / 2,
      y: p.y - GAP / 2,
      w: pick(widths, i) + GAP,
      h: pick(heights, i) + GAP,
    }));
    const clash = rects.some((a, i) =>
      rects.some((b, j) => j > i && overlaps(a, b)),
    );
    if (!clash) return pts;
    r *= 1.15;
  }
}
