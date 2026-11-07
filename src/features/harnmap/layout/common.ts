import type { Loop } from "../../../bindings/Loop";
import type { MapPhase } from "../../../bindings/MapPhase";
import type { Placement } from "../../../bindings/Placement";
import type { Shape } from "../../../bindings/Shape";
import type { Stage } from "../../../bindings/Stage";
import type { Zone } from "../../../bindings/Zone";

/** Pixel constants shared by every shape. */
export const NODE_W = 128;
export const NODE_H = 44;
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
  labelAt: Point;
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
  /** The line the stage lights sit on, through the stage headers. */
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

export function frameWidth(nodeW: number): number {
  return nodeW + 2 * FRAME_PAD;
}

export function frameHeight(count: number, nodeH: number): number {
  const stack = count === 0 ? 0 : count * (nodeH + GAP) - GAP;
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
  place: (heights: number[]) => Point[],
  extras: (frames: StageFrame[], ctx: Context) => Extras,
  topExtra = 0,
): Layout {
  const { w: nw, h: nh } = size(input);
  const st = stacks(input);
  const heights = stageHeights(input);
  const fw = frameWidth(nw);
  const raw = place(heights);
  const stageTop = PAD + (input.hasHooks ? HOOK_H + GAP * 2 : 0) + topExtra;
  const dx = raw.length === 0 ? 0 : PAD - Math.min(...raw.map((p) => p.x));
  const dy = raw.length === 0 ? 0 : stageTop - Math.min(...raw.map((p) => p.y));

  const frames: StageFrame[] = input.stages.map((s, i) => {
    const frame = {
      x: pick(raw, i).x + dx,
      y: pick(raw, i).y + dy,
      w: fw,
      h: pick(heights, i),
    };
    return {
      id: s.id,
      label: ellipsize(s.name),
      frame,
      header: { x: frame.x, y: frame.y, w: fw, h: HEADER_H },
    };
  });

  const nodes: NodeBox[] = [];
  frames.forEach((f, s) => {
    pick(st.stages, s).forEach((id, i) => {
      nodes.push({
        id,
        zone: { type: "stage", id: f.id },
        x: f.frame.x + FRAME_PAD,
        y: f.frame.y + HEADER_H + FRAME_PAD + i * (nh + GAP),
        w: nw,
        h: nh,
      });
    });
  });

  const contentRight =
    frames.length === 0
      ? PAD
      : Math.max(...frames.map((f) => f.frame.x + f.frame.w));
  const width = Math.max(MIN_WIDTH, contentRight + PAD);
  const bottom =
    frames.length === 0
      ? stageTop
      : Math.max(...frames.map((f) => f.frame.y + f.frame.h));
  const ctx: Context = { top: stageTop, bottom };
  const ex = extras(frames, ctx);

  const loops = drawLoops(input.loops, frames, bottom, ex.loopsInside === true);
  const loopSpace =
    loops.length === 0 || ex.loopsInside
      ? 0
      : Math.max(...loops.map((l) => l.c1.y - bottom)) * 0.75 + GAP * 2;

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

  const heads = frames.map((f) => center(f.header));
  const beam =
    ex.closedBeam && heads.length > 0 ? [...heads, pick(heads, 0)] : heads;
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

function drawLoops(
  loops: Loop[],
  frames: StageFrame[],
  bottom: number,
  inside: boolean,
): LoopArc[] {
  const index = new Map(frames.map((f, i) => [f.id, i]));
  const heads = frames.map((f) => center(f.header));
  const middle: Point = {
    x: heads.reduce((s, p) => s + p.x, 0) / (heads.length || 1),
    y: heads.reduce((s, p) => s + p.y, 0) / (heads.length || 1),
  };
  const arcs: LoopArc[] = [];
  for (const l of loops) {
    const a = index.get(l.from);
    const b = index.get(l.to);
    if (a === undefined || b === undefined) continue;
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
      const span = Math.abs(a - b);
      const depth = 28 + 14 * span;
      const spread = a === b ? 30 : 0;
      start = {
        x: pick(frames, a).frame.x + pick(frames, a).frame.w / 2,
        y: bottom,
      };
      end = {
        x: pick(frames, b).frame.x + pick(frames, b).frame.w / 2,
        y: bottom,
      };
      c1 = { x: start.x + spread, y: bottom + depth };
      c2 = { x: end.x - spread, y: bottom + depth };
    }
    arcs.push({
      from: l.from,
      to: l.to,
      label: l.label,
      start,
      c1,
      c2,
      end,
      labelAt: bezierMid(start, c1, c2, end),
    });
  }
  return arcs;
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
