import {
  pick,
  assemble,
  center,
  frameWidth,
  GAP,
  size,
  type Layout,
  type LayoutInput,
  type Point,
} from "./common";

const AMPLITUDE = 80;

/** The stage where the two diamonds meet: `define` in the design template, else the middle one. */
export function pinchIndex(input: LayoutInput): number {
  const named = input.stages.findIndex((s) => s.id === "define");
  return named >= 0 ? named : Math.floor((input.stages.length - 1) / 2);
}

/**
 * Two diamonds side by side, first from the first stage to the pinch, then from the pinch to the last.
 * Stages sit on the edges, alternating above and below the axis, and the pinch is on it.
 */
export function layoutDiamond(input: LayoutInput): Layout {
  const pitch = frameWidth(size(input).w) + GAP;
  const n = input.stages.length;
  const p = pinchIndex(input);
  return assemble(
    input,
    (heights) =>
      heights.map((_, i) => {
        const start = i <= p ? 0 : p;
        const end = i <= p ? p : n - 1;
        const t = end === start ? 0 : (i - start) / (end - start);
        const sign = (i - start) % 2 === 0 ? -1 : 1;
        return {
          x: i * pitch,
          y: Math.round(sign * AMPLITUDE * Math.sin(Math.PI * t)),
        };
      }),
    (frames) => {
      const heads = frames.map((f) => center(f.header));
      const outline = (a: number, b: number): Point[] => {
        const ys = heads.slice(a, b + 1).map((h) => h.y);
        const x = (pick(heads, a).x + pick(heads, b).x) / 2;
        return [
          pick(heads, a),
          { x, y: Math.min(...ys) - GAP },
          pick(heads, b),
          { x, y: Math.max(...ys) + GAP },
        ];
      };
      const outlines: Point[][] = [];
      if (p > 0) outlines.push(outline(0, p));
      if (p < n - 1) outlines.push(outline(p, n - 1));
      return { outlines };
    },
  );
}
