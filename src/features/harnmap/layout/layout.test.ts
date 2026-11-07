import { describe, expect, it } from "vitest";
import type { Shape } from "../../../bindings/Shape";
import {
  ellipsize,
  layout,
  layoutCycle,
  layoutDiamond,
  layoutLine,
  layoutParallel,
  overlaps,
  pick,
  type Layout,
  type LayoutInput,
} from "./index";

const SHAPES: Shape[] = ["line", "cycle", "diamond", "parallel"];

function input(
  shape: Shape,
  stages: number,
  per: number,
  extra: Partial<LayoutInput> = {},
): LayoutInput {
  const ids = Array.from({ length: stages }, (_, i) => `s${i}`);
  const placements: LayoutInput["placements"] = ids.flatMap((id, s) =>
    Array.from({ length: per }, (_, k) => ({
      artifact: `a${s}-${k}`,
      zone: { type: "stage" as const, id },
      order: per - k,
    })),
  );
  placements.push(
    ...Array.from({ length: 10 }, (_, k) => ({
      artifact: `t${k}`,
      zone: { type: "toolbelt" as const },
      order: k,
    })),
    ...Array.from({ length: 3 }, (_, k) => ({
      artifact: `f${k}`,
      zone: { type: "floor" as const },
      order: k,
    })),
  );
  return {
    shape,
    stages: ids.map((id) => ({
      id,
      name: `Stage number ${id} with a very long name`,
    })),
    loops:
      stages > 2
        ? [{ from: pick(ids, stages - 1), to: pick(ids, 0), label: "again" }]
        : [],
    phases: [],
    parallel: stages > 3 ? [pick(ids, 1), pick(ids, 2)] : [],
    placements,
    bundles: [],
    hasHooks: true,
    ...extra,
  };
}

function noOverlap(l: Layout) {
  for (let i = 0; i < l.nodes.length; i++) {
    for (let j = i + 1; j < l.nodes.length; j++) {
      expect(
        overlaps(pick(l.nodes, i), pick(l.nodes, j)),
        `${pick(l.nodes, i).id} vs ${pick(l.nodes, j).id}`,
      ).toBe(false);
    }
    for (const s of l.stages)
      expect(overlaps(pick(l.nodes, i), s.header)).toBe(false);
  }
  for (let i = 0; i < l.stages.length; i++) {
    for (let j = i + 1; j < l.stages.length; j++) {
      expect(overlaps(pick(l.stages, i).frame, pick(l.stages, j).frame)).toBe(
        false,
      );
    }
  }
  expect(overlaps(l.toolbelt, l.floor)).toBe(false);
}

describe.each(SHAPES)("%s", (shape) => {
  for (const stages of [1, 7, 12]) {
    for (const per of [0, 3, 10]) {
      it(`${stages} stages x ${per} stations: nothing overlaps, everything is placed`, () => {
        const i = input(shape, stages, per);
        const l = layout(i);
        noOverlap(l);
        expect(l.nodes).toHaveLength(stages * per + 13);
        expect(l.stages).toHaveLength(stages);
        for (const n of l.nodes) {
          expect(n.x).toBeGreaterThanOrEqual(0);
          expect(n.y).toBeGreaterThanOrEqual(0);
          expect(n.x + n.w).toBeLessThanOrEqual(l.width);
          expect(n.y + n.h).toBeLessThanOrEqual(l.height);
        }
        expect(l.hookRail).not.toBeNull();
        expect(l.beam.length).toBeGreaterThanOrEqual(stages);
      });
    }
  }

  it("is deterministic and does not change its input", () => {
    const i = input(shape, 7, 4);
    const copy = JSON.stringify(i);
    expect(layout(i)).toEqual(layout(i));
    expect(JSON.stringify(i)).toBe(copy);
  });

  it("orders stations by order then id, and drops repeats and unknown stages", () => {
    const i = input(shape, 2, 0, {
      placements: [
        { artifact: "b", zone: { type: "stage", id: "s0" }, order: 1 },
        { artifact: "a", zone: { type: "stage", id: "s0" }, order: 1 },
        { artifact: "c", zone: { type: "stage", id: "s0" }, order: 0 },
        { artifact: "c", zone: { type: "floor" }, order: 0 },
        { artifact: "x", zone: { type: "stage", id: "nope" }, order: 0 },
        { artifact: "a", zone: { type: "toolbelt" }, order: 5 },
      ],
    });
    const l = layout(i);
    expect(
      l.nodes.filter((n) => n.zone.type === "stage").map((n) => n.id),
    ).toEqual(["c", "a", "b"]);
    expect(l.nodes).toHaveLength(3);
  });

  it("places loops, the hook rail and the bands", () => {
    const l = layout(input(shape, 5, 2));
    expect(l.loops).toHaveLength(1);
    const arc = pick(l.loops, 0);
    expect(
      [arc.start, arc.c1, arc.c2, arc.end, arc.labelAt].every((p) =>
        Number.isFinite(p.x + p.y),
      ),
    ).toBe(true);
    expect(l.hookRail).toEqual({ x: 24, y: 24, w: l.width - 48, h: 28 });
    expect(l.toolbelt.y).toBeGreaterThan(
      Math.max(...l.stages.map((s) => s.frame.y + s.frame.h)),
    );
    expect(l.floor.y).toBeGreaterThan(l.toolbelt.y + l.toolbelt.h);
    expect(l.height).toBeGreaterThan(l.floor.y + l.floor.h);
    for (const s of l.stages)
      expect(overlaps(s.frame, l.hookRail!)).toBe(false);
  });

  it("has no hook rail without hooks, and skips loops that point nowhere", () => {
    const l = layout(
      input(shape, 4, 1, {
        hasHooks: false,
        loops: [
          { from: "s0", to: "missing", label: "x" },
          { from: "missing", to: "s0", label: "y" },
        ],
      }),
    );
    expect(l.hookRail).toBeNull();
    expect(l.loops).toEqual([]);
  });

  it("draws a loop from a stage to itself", () => {
    const l = layout(
      input(shape, 3, 1, { loops: [{ from: "s1", to: "s1", label: "retry" }] }),
    );
    expect(l.loops).toHaveLength(1);
  });

  it("groups bundle members that share a stage and skips the others", () => {
    const l = layout(
      input(shape, 3, 4, {
        bundles: [["a0-0", "a0-1", "a1-0", "t0"], ["a2-0", "a1-1"], ["nope"]],
      }),
    );
    expect(l.bundles).toHaveLength(1);
    const g = pick(l.bundles, 0);
    expect(g.bundle).toBe(0);
    expect(g.stage).toBe("s0");
    expect(g.members.sort()).toEqual(["a0-0", "a0-1"]);
    for (const id of g.members) {
      const n = l.nodes.find((x) => x.id === id)!;
      expect(n.x).toBeGreaterThan(g.rect.x);
      expect(n.x + n.w).toBeLessThan(g.rect.x + g.rect.w);
      expect(n.y).toBeGreaterThan(g.rect.y);
      expect(n.y + n.h).toBeLessThan(g.rect.y + g.rect.h);
    }
  });

  it("makes a group per stage for a bundle spread over stages", () => {
    const l = layout(
      input(shape, 2, 2, { bundles: [["a0-0", "a0-1", "a1-0", "a1-1"]] }),
    );
    expect(l.bundles.map((b) => b.stage)).toEqual(["s0", "s1"]);
  });

  it("wraps band nodes in rows and honours a custom node size", () => {
    const l = layout(input(shape, 1, 0, { nodeSize: { w: 100, h: 30 } }));
    expect(l.nodes.every((n) => n.w === 100 && n.h === 30)).toBe(true);
    expect(l.toolbelt.h).toBeGreaterThan(30 + 16);
    noOverlap(l);
  });

  it("handles a map with no stages", () => {
    const l = layout(input(shape, 0, 0, { loops: [], parallel: [] }));
    expect(l.stages).toEqual([]);
    expect(l.beam).toEqual([]);
    expect(l.nodes).toHaveLength(13);
  });

  it("ellipsises stage names", () => {
    const l = layout(input(shape, 1, 0));
    expect(pick(l.stages, 0).label).toHaveLength(18);
    expect(pick(l.stages, 0).label.endsWith("…")).toBe(true);
  });
});

describe("dispatcher", () => {
  it("calls the layout of each shape", () => {
    expect(layout(input("line", 3, 1))).toEqual(
      layoutLine(input("line", 3, 1)),
    );
    expect(layout(input("cycle", 3, 1))).toEqual(
      layoutCycle(input("cycle", 3, 1)),
    );
    expect(layout(input("diamond", 3, 1))).toEqual(
      layoutDiamond(input("diamond", 3, 1)),
    );
    expect(layout(input("parallel", 3, 1))).toEqual(
      layoutParallel(input("parallel", 3, 1)),
    );
  });
});

describe("pick", () => {
  it("returns the item and throws on a miss", () => {
    expect(pick([1, 2], 1)).toBe(2);
    expect(() => pick([1], 3)).toThrow("no item at 3");
  });
});

describe("ellipsize", () => {
  it("keeps short text and cuts long text", () => {
    expect(ellipsize("short")).toBe("short");
    expect(ellipsize("abcdef", 4)).toBe("abc…");
  });
});

describe("line", () => {
  it("puts stages left to right on a 144 px pitch with a straight beam", () => {
    const l = layoutLine(input("line", 4, 2));
    expect(l.stages.map((s) => s.frame.x)).toEqual([24, 168, 312, 456]);
    expect(new Set(l.stages.map((s) => s.frame.y)).size).toBe(1);
    expect(new Set(l.beam.map((p) => p.y)).size).toBe(1);
    expect(l.forkJoin).toBeNull();
    expect(l.returnArrow).toBeNull();
  });
});

describe("cycle", () => {
  it("closes the beam and gives a return arrow, with loops inside", () => {
    const l = layoutCycle(input("cycle", 6, 1));
    expect(l.beam).toHaveLength(7);
    expect(pick(l.beam, 6)).toEqual(pick(l.beam, 0));
    expect(l.returnArrow).not.toBeNull();
    expect(pick(l.loops, 0).c1).toEqual(pick(l.loops, 0).c2);
    expect(pick(l.stages, 0).frame.y).toBeLessThan(pick(l.stages, 3).frame.y);
  });
});

describe("diamond", () => {
  it("pinches at define in the design template", () => {
    const stages = ["brief", "explore", "define", "build", "ship"].map(
      (id) => ({ id, name: id }),
    );
    const l = layoutDiamond({
      ...input("diamond", 0, 0),
      stages,
      loops: [],
      parallel: [],
    });
    expect(l.outlines).toHaveLength(2);
    const pinch = pick(l.stages, 2).header.y;
    expect(pick(l.stages, 0).header.y).toBe(pinch);
    expect(pick(l.stages, 4).header.y).toBe(pinch);
    expect(pick(l.stages, 1).header.y).not.toBe(pinch);
    expect(pick(pick(l.outlines, 0), 2)).toEqual(pick(pick(l.outlines, 1), 0));
  });

  it("pinches in the middle of a generic map", () => {
    const l = layoutDiamond(input("diamond", 7, 0));
    expect(pick(l.stages, 3).header.y).toBe(pick(l.stages, 0).header.y);
    expect(l.outlines).toHaveLength(2);
  });

  it("draws one diamond when the pinch is the first or last stage", () => {
    const stages = (ids: string[]) => ids.map((id) => ({ id, name: id }));
    const base = { ...input("diamond", 0, 0), loops: [], parallel: [] };
    expect(
      layoutDiamond({ ...base, stages: stages(["define", "x", "y"]) }).outlines,
    ).toHaveLength(1);
    expect(
      layoutDiamond({ ...base, stages: stages(["x", "y", "define"]) }).outlines,
    ).toHaveLength(1);
    expect(
      layoutDiamond({ ...base, stages: stages(["x"]) }).outlines,
    ).toHaveLength(0);
  });
});

describe("parallel", () => {
  it("stacks the parallel stages in one column between a fork and a join", () => {
    const l = layoutParallel(input("parallel", 5, 2));
    const a = pick(l.stages, 1);
    const b = pick(l.stages, 2);
    expect(a.frame.x).toBe(b.frame.x);
    expect(b.frame.y).toBeGreaterThan(a.frame.y + a.frame.h);
    expect(pick(l.stages, 3).frame.x).toBe(a.frame.x + 144);
    expect(l.forkJoin?.tracks).toHaveLength(2);
    expect(l.forkJoin!.fork.x).toBeLessThan(a.frame.x);
    expect(l.forkJoin!.join.x).toBeGreaterThan(a.frame.x + a.frame.w);
  });

  it("has no fork without parallel stages", () => {
    expect(layoutParallel(input("parallel", 3, 1)).forkJoin).toBeNull();
  });

  it("puts phases behind their stages as zones with a label row", () => {
    const l = layoutParallel(
      input("parallel", 5, 2, {
        phases: [
          { name: "Pre", stages: ["s0", "s1", "s2"] },
          { name: "Post", stages: ["s3", "s4", "gone"] },
          { name: "Empty", stages: ["gone"] },
        ],
      }),
    );
    expect(l.zones.map((z) => z.name)).toEqual(["Pre", "Post"]);
    const pre = pick(l.zones, 0);
    const post = pick(l.zones, 1);
    expect(pre.rect.x + pre.rect.w).toBeLessThan(post.rect.x);
    for (const s of l.stages) {
      const z = s.id <= "s2" ? pre : post;
      expect(s.frame.x).toBeGreaterThan(z.rect.x);
      expect(s.frame.x + s.frame.w).toBeLessThan(z.rect.x + z.rect.w);
      expect(s.frame.y + s.frame.h).toBeLessThan(z.rect.y + z.rect.h);
    }
    expect(pick(l.stages, 0).frame.y).toBeGreaterThanOrEqual(pre.rect.y + 24);
    expect(l.hookRail!.y + l.hookRail!.h).toBeLessThanOrEqual(pre.rect.y);
  });
});
