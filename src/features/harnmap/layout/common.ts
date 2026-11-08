import type { Loop } from "../../../bindings/Loop";
import type { MapPhase } from "../../../bindings/MapPhase";
import type { Placement } from "../../../bindings/Placement";
import type { Shape } from "../../../bindings/Shape";
import type { Stage } from "../../../bindings/Stage";
import type { Zone } from "../../../bindings/Zone";

/** Pixel constants shared by every shape. */
export const NODE_W = 176;
export const NODE_H = 52;
/** A stage with more stations than this stacks them in two columns, so one busy stage doesn't stretch the map. */
export const MAX_STACK = 6;
export const GAP = 8;
export const PAD = 24;
export const FRAME_PAD = 4;
export const HEADER_H = 36;
export const HOOK_H = 28;
export const ZONE_LABEL_H = 24;
export const BAND_GAP = 24;
export const BAND_PAD = 8;
export const MIN_WIDTH = 480;
/** Stage names longer than this are cut and end in an ellipsis (`ellipsize`). */
export const MAX_LABEL_CHARS = 18;

/** The renderer writes this after the loop's name; the label box is sized for it. */
export const LOOP_SUFFIX = " · Claude’s reading";
/** Estimated text width per character at the canvas label font (10.5 px sans). */
export const LABEL_CHAR_W = 6.5;
export const LABEL_H = 16;
const LABEL_PAD = 8;
/** Vertical distance between the arcs of nested loops. */
const LOOP_LEVEL = 32;
const LABEL_STEP = 24;
const LABEL_MARGIN = 4;

export interface Point {
  x: number;
  y: number;
}

export interface Rect extends Point {
  w: number;
  h: number;
}

/** What a layout needs from a map: the caller has already dropped unplaced and hidden artifacts. */
export interface LayoutInput {
  shape: Shape;
  stages: Array<Pick<Stage, "id" | "name">>;
  loops: Loop[];
  phases: MapPhase[];
  /** Stage ids drawn as tracks between a fork and a join (shape `parallel`). */
  parallel: string[];
  placements: Array<Pick<Placement, "artifact" | "zone" | "order">>;
  /** Artifact ids per bundle. */
  bundles: string[][];
  hasHooks: boolean;
  nodeSize?: { w: number; h: number };
}

export interface StageFrame {
  id: string;
  /** The name cut to `MAX_LABEL_CHARS`. */
  label: string;
  frame: Rect;
  header: Rect;
}

export interface NodeBox extends Rect {
  id: string;
  zone: Zone;
}

export interface LoopArc {
  from: string;
  to: string;
  label: string;
  /** Cubic Bezier: start, two control points, end. */
  start: Point;
  c1: Point;
  c2: Point;
  end: Point;
  /** Top centre of `labelBox`. */
  labelAt: Point;
  /** Where the label text sits: it overlaps no other label, loop path or stage frame. */
  labelBox: Rect;
}

export interface BundleGroup {
  /** Index in `LayoutInput.bundles`. */
  bundle: number;
  stage: string;
  rect: Rect;
  members: string[];
}

export interface PhaseZone {
  name: string;
  rect: Rect;
}

export interface ForkJoin {
  fork: Point;
  join: Point;
  /** One segment per track, from the fork side to the join side of its header. */
  tracks: Array<{ a: Point; b: Point }>;
}

export interface ReturnArrow {
  from: Point;
  to: Point;
  /** Control point of the quadratic curve that closes the cycle. */
  via: Point;
}

export interface Layout {
  shape: Shape;
  width: number;
  height: number;
  stages: StageFrame[];
  nodes: NodeBox[];
  loops: LoopArc[];
  hookRail: Rect | null;
  bundles: BundleGroup[];
  toolbelt: Rect;
  floor: Rect;
  /** The rail through the top edge of every stage frame. */
  beam: Point[];
  zones: PhaseZone[];
  forkJoin: ForkJoin | null;
  /** Diamond only: each diamond as four corner points (left, top, right, bottom). */
  outlines: Point[][];
  returnArrow: ReturnArrow | null;
}

export interface Extras {
  zones?: PhaseZone[];
  forkJoin?: ForkJoin;
  outlines?: Point[][];
  returnArrow?: ReturnArrow;
  /** Loops are drawn inside the shape instead of below it. */
  loopsInside?: boolean;
  closedBeam?: boolean;
}

export interface Context {
  /** Y of the top of the stage frames. */
  top: number;
  /** Y of the bottom of the lowest stage frame. */
  bottom: number;
}

/** Cuts text to `max` characters, the last one an ellipsis. */
export function ellipsize(text: string, max: number = MAX_LABEL_CHARS): string {
  return text.length <= max ? text : `${text.slice(0, max - 1)}…`;
}

/** The item at `i`; the layouts only ask for indexes they built, so a miss is a bug. */
export function pick<T>(items: T[], i: number): T {
  const item = items[i];
  if (item === undefined) throw new Error(`no item at ${i}`);
  return item;
}

/** How many columns a stage with `count` stations uses. */
export function columnsFor(count: number): number {
  return count > MAX_STACK ? 2 : 1;
}

/** Rows per column for `count` stations. */
function rowsFor(count: number): number {
  return Math.ceil(count / columnsFor(count));
}

export function frameWidth(nodeW: number, count = 0): number {
  const cols = columnsFor(count);
  return cols * nodeW + (cols - 1) * GAP + 2 * FRAME_PAD;
}

export function frameHeight(count: number, nodeH: number): number {
  const rows = rowsFor(count);
  const stack = rows === 0 ? 0 : rows * (nodeH + GAP) - GAP;
  return HEADER_H + FRAME_PAD + stack + FRAME_PAD;
}

export function size(input: LayoutInput): { w: number; h: number } {
  return input.nodeSize ?? { w: NODE_W, h: NODE_H };
}

export function overlaps(a: Rect, b: Rect): boolean {
  return (
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
  );
}

/** Stage heights (header and stack) in stage order, for the shapes to place. */
export function stageHeights(input: LayoutInput): number[] {
  return stacks(input).stages.map((ids) =>
    frameHeight(ids.length, size(input).h),
  );
}

/** Stage frame widths in stage order: one column, or two for a busy stage. */
export function stageWidths(input: LayoutInput): number[] {
  return stacks(input).stages.map((ids) =>
    frameWidth(size(input).w, ids.length),
  );
}

/** Left edges for frames of `widths` laid left to right, `GAP` apart. */
export function rowOf(widths: number[]): number[] {
  const xs: number[] = [];
  let x = 0;
  for (const w of widths) {
    xs.push(x);
    x += w + GAP;
  }
  return xs;
}

interface Stacks {
  /** Per stage, in stage order. */
  stages: string[][];
  toolbelt: string[];
  floor: string[];
}

/** Artifact ids per zone, by `order` then id; a repeated artifact counts once, an unknown stage is dropped. */
function stacks(input: LayoutInput): Stacks {
  const out: Stacks = {
    stages: input.stages.map(() => []),
    toolbelt: [],
    floor: [],
  };
  const index = new Map<string, number>();
  input.stages.forEach((s, i) => index.has(s.id) || index.set(s.id, i));
  const sorted = [...input.placements].sort(
    (a, b) =>
      a.order - b.order ||
      (a.artifact < b.artifact ? -1 : a.artifact > b.artifact ? 1 : 0),
  );
  const seen = new Set<string>();
  for (const p of sorted) {
    if (seen.has(p.artifact)) continue;
    seen.add(p.artifact);
    if (p.zone.type === "stage") {
      const at = index.get(p.zone.id);
      if (at !== undefined) pick(out.stages, at).push(p.artifact);
    } else if (p.zone.type === "toolbelt") out.toolbelt.push(p.artifact);
    else out.floor.push(p.artifact);
  }
  return out;
}

function center(r: Rect): Point {
  return { x: r.x + r.w / 2, y: r.y + r.h / 2 };
}

function bezierMid(a: Point, b: Point, c: Point, d: Point): Point {
  return {
    x: (a.x + 3 * b.x + 3 * c.x + d.x) / 8,
    y: (a.y + 3 * b.y + 3 * c.y + d.y) / 8,
  };
}

function bounds(rects: Rect[]): Rect {
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  return {
    x,
    y,
    w: Math.max(...rects.map((r) => r.x + r.w)) - x,
    h: Math.max(...rects.map((r) => r.y + r.h)) - y,
  };
}

/**
 * Builds the whole layout from the top-left corners a shape chose for its stage frames.
 * `place` gets the frame heights; `extras` gets the final frames, so its geometry needs no shifting.
 */
export function assemble(
  input: LayoutInput,
  place: (heights: number[], widths: number[]) => Point[],
  extras: (frames: StageFrame[], ctx: Context) => Extras,
  topExtra = 0,
): Layout {
  const { w: nw, h: nh } = size(input);
  const st = stacks(input);
  const heights = stageHeights(input);
  const widths = stageWidths(input);
  const raw = place(heights, widths);
  const stageTop = PAD + (input.hasHooks ? HOOK_H + GAP * 2 : 0) + topExtra;
  const dx = raw.length === 0 ? 0 : PAD - Math.min(...raw.map((p) => p.x));
  const dy = raw.length === 0 ? 0 : stageTop - Math.min(...raw.map((p) => p.y));

  const frames: StageFrame[] = input.stages.map((s, i) => {
    const frame = {
      x: pick(raw, i).x + dx,
      y: pick(raw, i).y + dy,
      w: pick(widths, i),
      h: pick(heights, i),
    };
    return {
      id: s.id,
      label: ellipsize(s.name),
      frame,
      header: { x: frame.x, y: frame.y, w: frame.w, h: HEADER_H },
    };
  });

  const nodes: NodeBox[] = [];
  frames.forEach((f, s) => {
    const ids = pick(st.stages, s);
    const rows = rowsFor(ids.length);
    // Column by column: the first column reads top to bottom, then the second.
    ids.forEach((id, i) => {
      nodes.push({
        id,
        zone: { type: "stage", id: f.id },
        x: f.frame.x + FRAME_PAD + Math.floor(i / rows) * (nw + GAP),
        y: f.frame.y + HEADER_H + FRAME_PAD + (i % rows) * (nh + GAP),
        w: nw,
        h: nh,
      });
    });
  });

  const contentRight =
    frames.length === 0
      ? PAD
      : Math.max(...frames.map((f) => f.frame.x + f.frame.w));
  const bottom =
    frames.length === 0
      ? stageTop
      : Math.max(...frames.map((f) => f.frame.y + f.frame.h));
  const ctx: Context = { top: stageTop, bottom };
  const ex = extras(frames, ctx);

  const loops = drawLoops(
    input.loops,
    frames,
    bottom,
    ex.loopsInside === true,
    stageTop,
  );
  const lowest = Math.max(
    bottom,
    ...loops.flatMap((l) => [
      l.labelBox.y + l.labelBox.h,
      ...curve(l).map((p) => p.y),
    ]),
  );
  const loopSpace = lowest === bottom ? 0 : lowest - bottom + GAP * 2;
  const width = Math.max(
    MIN_WIDTH,
    contentRight + PAD,
    ...loops.map((l) => l.labelBox.x + l.labelBox.w + PAD),
  );

  const bandW = width - 2 * PAD;
  const toolbelt = band(
    PAD,
    bottom + loopSpace + BAND_GAP,
    bandW,
    st.toolbelt,
    "toolbelt",
    nodes,
    nw,
    nh,
  );
  const floor = band(
    PAD,
    toolbelt.y + toolbelt.h + BAND_GAP,
    bandW,
    st.floor,
    "floor",
    nodes,
    nw,
    nh,
  );

  // The rail runs along the top edge of the stage frames, so it reads as one line through them.
  const tops = frames.map((f) => ({
    x: f.frame.x + f.frame.w / 2,
    y: f.frame.y,
  }));
  const beam =
    ex.closedBeam && tops.length > 0 ? [...tops, pick(tops, 0)] : tops;
  return {
    shape: input.shape,
    width,
    height: floor.y + floor.h + PAD,
    stages: frames,
    nodes,
    loops,
    hookRail: input.hasHooks ? { x: PAD, y: PAD, w: bandW, h: HOOK_H } : null,
    bundles: bundleGroups(input.bundles, nodes),
    toolbelt,
    floor,
    beam,
    zones: ex.zones ?? [],
    forkJoin: ex.forkJoin ?? null,
    outlines: ex.outlines ?? [],
    returnArrow: ex.returnArrow ?? null,
  };
}

/** A band as wide as the map, its nodes in rows; it keeps one row of height when empty. */
function band(
  x: number,
  y: number,
  w: number,
  ids: string[],
  zone: "toolbelt" | "floor",
  nodes: NodeBox[],
  nw: number,
  nh: number,
): Rect {
  const cols = Math.max(1, Math.floor((w - 2 * BAND_PAD + GAP) / (nw + GAP)));
  const rows = Math.max(1, Math.ceil(ids.length / cols));
  ids.forEach((id, i) => {
    nodes.push({
      id,
      zone: { type: zone },
      x: x + BAND_PAD + (i % cols) * (nw + GAP),
      y: y + BAND_PAD + Math.floor(i / cols) * (nh + GAP),
      w: nw,
      h: nh,
    });
  });
  return { x, y, w, h: 2 * BAND_PAD + rows * (nh + GAP) - GAP };
}

function bezierAt(l: LoopArc, t: number): Point {
  const u = 1 - t;
  const f = (a: number, b: number, c: number, d: number) =>
    u * u * u * a + 3 * u * u * t * b + 3 * u * t * t * c + t * t * t * d;
  return {
    x: f(l.start.x, l.c1.x, l.c2.x, l.end.x),
    y: f(l.start.y, l.c1.y, l.c2.y, l.end.y),
  };
}

/** The arc as 65 points, dense enough that no label box slips between two of them. */
function curve(l: LoopArc): Point[] {
  return Array.from({ length: 65 }, (_, i) => bezierAt(l, i / 64));
}

/** The label box the loop's text needs: its estimated width, `LABEL_H` high. */
export function labelSize(label: string): { w: number; h: number } {
  return {
    w: (label.length + LOOP_SUFFIX.length) * LABEL_CHAR_W + LABEL_PAD,
    h: LABEL_H,
  };
}

/**
 * Puts each label under its arc's lowest point, or the middle of an inside loop, shortest span first.
 * A label that would touch an earlier label, another loop's path or a stage frame moves sideways, then
 * down a row at a time until it is clear; the whole search is deterministic.
 */
function placeLabels(
  arcs: Array<Omit<LoopArc, "labelAt" | "labelBox">>,
  order: number[],
  frames: StageFrame[],
  top: number,
): LoopArc[] {
  const full = arcs.map((a) => ({
    ...a,
    labelAt: a.start,
    labelBox: { x: 0, y: 0, w: 0, h: 0 },
  }));
  const paths = full.map(curve);
  const taken: Rect[] = frames.map((f) => f.frame);
  for (const i of order) {
    const arc = pick(full, i);
    const { w, h } = labelSize(arc.label);
    const mid = bezierMid(arc.start, arc.c1, arc.c2, arc.end);
    const clear = (r: Rect): boolean =>
      r.x >= LABEL_MARGIN &&
      r.y >= top &&
      !taken.some((t) => overlaps(r, t)) &&
      !paths.some(
        (path, j) =>
          j !== i &&
          path.some(
            (p) =>
              p.x > r.x - 2 &&
              p.x < r.x + r.w + 2 &&
              p.y > r.y - 2 &&
              p.y < r.y + r.h + 2,
          ),
      );
    let box: Rect | null = null;
    for (let row = 0; box === null; row++) {
      for (let k = 0; k < 9 && box === null; k++) {
        const shift = (k % 2 === 0 ? 1 : -1) * Math.ceil(k / 2) * LABEL_STEP;
        const r = {
          x: mid.x - w / 2 + shift,
          y: mid.y + 2 + row * (h + 2),
          w,
          h,
        };
        if (clear(r)) box = r;
      }
    }
    arc.labelBox = box;
    arc.labelAt = { x: box.x + box.w / 2, y: box.y };
    taken.push(box);
  }
  return full;
}

function drawLoops(
  loops: Loop[],
  frames: StageFrame[],
  bottom: number,
  inside: boolean,
  top: number,
): LoopArc[] {
  const index = new Map(frames.map((f, i) => [f.id, i]));
  const heads = frames.map((f) => center(f.header));
  const middle: Point = {
    x: heads.reduce((s, p) => s + p.x, 0) / (heads.length || 1),
    y: heads.reduce((s, p) => s + p.y, 0) / (heads.length || 1),
  };
  const valid = loops.flatMap((l) => {
    const a = index.get(l.from);
    const b = index.get(l.to);
    return a === undefined || b === undefined ? [] : [{ l, a, b }];
  });
  const sorted = order(valid.map((v) => Math.abs(v.a - v.b)));
  const arcs = valid.map(({ l, a, b }, n) => {
    let start: Point;
    let end: Point;
    let c1: Point;
    let c2: Point;
    if (inside) {
      start = pick(heads, a);
      end = pick(heads, b);
      c1 = middle;
      c2 = middle;
    } else {
      // Shorter spans nest inside longer ones; each rank adds one level of depth.
      const depth = 28 + 14 * Math.abs(a - b) + LOOP_LEVEL * sorted.indexOf(n);
      const spread = a === b ? 30 : 0;
      // Each end leaves from its own stage's frame, so an arc never hangs in empty space.
      const fa = pick(frames, a).frame;
      const fb = pick(frames, b).frame;
      start = { x: fa.x + fa.w / 2, y: fa.y + fa.h };
      end = { x: fb.x + fb.w / 2, y: fb.y + fb.h };
      c1 = { x: start.x + spread, y: bottom + depth };
      c2 = { x: end.x - spread, y: bottom + depth };
    }
    return { from: l.from, to: l.to, label: l.label, start, c1, c2, end };
  });
  return placeLabels(arcs, sorted, frames, top);
}

/** Loop indexes, shortest span first, ties in input order. */
function order(spans: number[]): number[] {
  return spans
    .map((span, i) => ({ span, i }))
    .sort((a, b) => a.span - b.span || a.i - b.i)
    .map((e) => e.i);
}

const BUNDLE_MARGIN = 3;

/** One rectangle per bundle and stage around the bundle's nodes there; a lone member makes no group. */
function bundleGroups(bundles: string[][], nodes: NodeBox[]): BundleGroup[] {
  const out: BundleGroup[] = [];
  bundles.forEach((ids, bundle) => {
    const byStage = new Map<string, NodeBox[]>();
    for (const n of nodes) {
      if (n.zone.type === "stage" && ids.includes(n.id)) {
        byStage.set(n.zone.id, [...(byStage.get(n.zone.id) ?? []), n]);
      }
    }
    for (const [stage, members] of byStage) {
      if (members.length < 2) continue;
      const b = bounds(members);
      out.push({
        bundle,
        stage,
        rect: {
          x: b.x - BUNDLE_MARGIN,
          y: b.y - BUNDLE_MARGIN,
          w: b.w + 2 * BUNDLE_MARGIN,
          h: b.h + 2 * BUNDLE_MARGIN,
        },
        members: members.map((m) => m.id),
      });
    }
  });
  return out;
}

/** Zones for the phases: the box around the frames of each phase's stages, with room for its label. */
export function phaseZones(
  phases: MapPhase[],
  frames: StageFrame[],
  ctx: Context,
): PhaseZone[] {
  const zones: PhaseZone[] = [];
  for (const p of phases) {
    const mine = frames
      .filter((f) => p.stages.includes(f.id))
      .map((f) => f.frame);
    if (mine.length === 0) continue;
    const b = bounds(mine);
    const y = ctx.top - ZONE_LABEL_H;
    zones.push({
      name: p.name,
      rect: { x: b.x - 3, y, w: b.w + 6, h: ctx.bottom + GAP - y },
    });
  }
  return zones;
}

export { center };
