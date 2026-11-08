import { describe, expect, it } from "vitest";
import type { Shape } from "../../../bindings/Shape";
import type { Zone } from "../../../bindings/Zone";
import {
  ellipsize,
  FRAME_PAD,
  GAP,
  MAX_STACK,
  NODE_W,
  layout,
  layoutCycle,
  layoutDiamond,
  layoutLine,
  layoutParallel,
  LOOP_SUFFIX,
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
  it("puts stages left to right, one frame and a gap apart, with a straight beam", () => {
    const l = layoutLine(input("line", 4, 2));
    const pitch = NODE_W + 2 * FRAME_PAD + GAP;
    expect(l.stages.map((s) => s.frame.x)).toEqual([
      24,
      24 + pitch,
      24 + 2 * pitch,
      24 + 3 * pitch,
    ]);
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
    expect(pick(l.stages, 3).frame.x).toBe(a.frame.x + a.frame.w + GAP);
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

function curvePoints(a: Layout["loops"][number]) {
  return Array.from({ length: 401 }, (_, i) => {
    const t = i / 400;
    const u = 1 - t;
    const f = (p0: number, p1: number, p2: number, p3: number) =>
      u * u * u * p0 + 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t * p3;
    return {
      x: f(a.start.x, a.c1.x, a.c2.x, a.end.x),
      y: f(a.start.y, a.c1.y, a.c2.y, a.end.y),
    };
  });
}

function labelsClear(l: Layout) {
  l.loops.forEach((a, i) => {
    const box = a.labelBox;
    expect(box.w).toBeGreaterThanOrEqual(
      (a.label.length + LOOP_SUFFIX.length) * 6.5,
    );
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.x + box.w).toBeLessThanOrEqual(l.width);
    expect(box.y + box.h).toBeLessThanOrEqual(l.toolbelt.y);
    for (const s of l.stages) expect(overlaps(box, s.frame)).toBe(false);
    l.loops.forEach((b, j) => {
      if (j > i) expect(overlaps(box, b.labelBox), `${i} vs ${j}`).toBe(false);
      if (j !== i)
        for (const p of curvePoints(b)) {
          const hit =
            p.x > box.x &&
            p.x < box.x + box.w &&
            p.y > box.y &&
            p.y < box.y + box.h;
          expect(hit, `label ${i} on path ${j}`).toBe(false);
        }
    });
  });
}

describe.each(SHAPES)("%s loop labels", (shape) => {
  const lab = (from: number, to: number, label: string) => ({
    from: `s${from}`,
    to: `s${to}`,
    label,
  });
  const cases: Array<[string, LayoutInput["loops"]]> = [
    [
      "two loops into one stage",
      [lab(2, 0, "tests fail"), lab(1, 0, "changes requested")],
    ],
    [
      "three loops into one stage",
      [
        lab(3, 0, "tests fail"),
        lab(2, 0, "changes requested"),
        lab(1, 0, "needs rework"),
      ],
    ],
    [
      "crossing loops",
      [lab(0, 2, "back and forth"), lab(1, 3, "crossing over")],
    ],
    [
      "opposite loops and a self loop",
      [lab(0, 1, "a"), lab(1, 0, "b"), lab(1, 1, "retry")],
    ],
  ];
  it.each(cases)("keeps %s apart", (_, loops) => {
    const l = layout(input(shape, 4, 2, { loops }));
    expect(l.loops).toHaveLength(loops.length);
    labelsClear(l);
    const arcs = l.loops.map((a) => `${a.c1.y}/${a.start.x}/${a.end.x}`);
    if (shape !== "cycle") expect(new Set(arcs).size).toBe(arcs.length);
  });

  it("moves the bands below the deepest label", () => {
    const loops = [lab(2, 0, "tests fail"), lab(1, 0, "changes requested")];
    const l = layout(input(shape, 3, 1, { loops }));
    for (const a of l.loops)
      expect(l.toolbelt.y).toBeGreaterThan(a.labelBox.y + a.labelBox.h);
  });
});

describe("line loop depth", () => {
  it("gives nested loops distinct depths and levels the outer one below the inner one", () => {
    const one = layout(input("line", 3, 1));
    const two = layout(
      input("line", 3, 1, {
        loops: [
          { from: "s2", to: "s0", label: "x" },
          { from: "s1", to: "s0", label: "y" },
        ],
      }),
    );
    expect(pick(two.loops, 0).c1.y - pick(one.loops, 0).c1.y).toBe(32);
    expect(pick(two.loops, 1).c1.y).toBeLessThan(pick(two.loops, 0).c1.y);
    expect(two.toolbelt.y).toBeGreaterThan(one.toolbelt.y);
  });
});

describe("busy stages and loop ends", () => {
  it("stacks a stage with more than MAX_STACK stations in two columns, read column by column", () => {
    const n = MAX_STACK + 4;
    for (const shape of SHAPES) {
      const l = layout(input(shape, 3, n));
      const frame = pick(l.stages, 0).frame;
      expect(frame.w).toBe(2 * NODE_W + GAP + 2 * FRAME_PAD);
      const mine = l.nodes.filter(
        (b) => b.zone.type === "stage" && b.zone.id === "s0",
      );
      expect(new Set(mine.map((b) => b.x)).size).toBe(2);
      const rows = Math.ceil(n / 2);
      // The first `rows` stations fill the left column, top to bottom.
      expect(new Set(mine.slice(0, rows).map((b) => b.x)).size).toBe(1);
      for (const b of mine) {
        expect(b.x + b.w).toBeLessThanOrEqual(frame.x + frame.w);
        expect(b.y + b.h).toBeLessThanOrEqual(frame.y + frame.h);
      }
    }
  });

  it("keeps a stage at MAX_STACK stations in one column", () => {
    const l = layoutLine(input("line", 3, MAX_STACK));
    expect(pick(l.stages, 0).frame.w).toBe(NODE_W + 2 * FRAME_PAD);
  });

  it("starts and ends each loop below the line at its own stage's frame", () => {
    const base = input("line", 4, 1);
    const l = layoutLine({
      ...base,
      placements: [
        ...base.placements,
        ...Array.from({ length: 5 }, (_, k) => ({
          artifact: `x${k}`,
          zone: { type: "stage" as const, id: "s2" },
          order: 10 + k,
        })),
      ],
      loops: [{ from: "s1", to: "s0", label: "back" }],
    });
    const loop = pick(l.loops, 0);
    const from = pick(l.stages, 1).frame;
    const to = pick(l.stages, 0).frame;
    expect(loop.start).toEqual({ x: from.x + from.w / 2, y: from.y + from.h });
    expect(loop.end).toEqual({ x: to.x + to.w / 2, y: to.y + to.h });
  });
});

describe.each(SHAPES)("%s: parent and child", (shape) => {
  const stage = (id: string) => ({ type: "stage" as const, id });
  const node = (l: Layout, id: string) => {
    const n = l.nodes.find((x) => x.id === id);
    if (!n) throw new Error(`no node ${id}`);
    return n;
  };
  const withKids = (
    extra: LayoutInput["placements"],
    zone: Zone = stage("s0"),
  ) =>
    input(shape, 2, 0, {
      placements: extra.map((p, i) => ({ ...p, zone, order: i })),
    });

  it("draws a child right under its parent, narrower and indented, with a connector", () => {
    const l = layout(
      withKids([
        { artifact: "cmd", zone: stage("s0"), order: 0 },
        { artifact: "other", zone: stage("s0"), order: 0 },
        { artifact: "skill", zone: stage("s0"), order: 0, parent: "cmd" },
      ]),
    );
    const cmd = node(l, "cmd");
    const skill = node(l, "skill");
    expect(skill.depth).toBe(1);
    expect(skill.parent).toBe("cmd");
    expect(skill.y).toBe(cmd.y + cmd.h + GAP);
    expect(skill.x).toBeGreaterThan(cmd.x);
    expect(skill.w).toBeLessThan(cmd.w);
    expect(skill.x + skill.w).toBe(cmd.x + cmd.w);
    const link = l.links.find((k) => k.child === "skill");
    expect(link?.parent).toBe("cmd");
    expect(link?.points[0]?.y).toBe(cmd.y + cmd.h);
    expect(link?.points.at(-1)).toEqual({
      x: skill.x,
      y: skill.y + skill.h / 2,
    });
    noOverlap(l);
  });

  it("draws two levels, each one step in, and no third", () => {
    const l = layout(
      withKids([
        { artifact: "cmd", zone: stage("s0"), order: 0 },
        { artifact: "skill", zone: stage("s0"), order: 0, parent: "cmd" },
        { artifact: "agent", zone: stage("s0"), order: 0, parent: "skill" },
        { artifact: "deep", zone: stage("s0"), order: 0, parent: "agent" },
      ]),
    );
    const [a, b, c, d] = ["cmd", "skill", "agent", "deep"].map((id) =>
      node(l, id),
    );
    expect([a?.depth, b?.depth, c?.depth, d?.depth]).toEqual([0, 1, 2, 2]);
    expect(c && b && c.x - b.x).toBe(b && a && b.x - a.x);
    expect(l.links).toHaveLength(3);
  });

  it("keeps a group in one column of a busy stage", () => {
    const solo = Array.from({ length: 4 }, (_, k) => ({
      artifact: `solo${k}`,
      order: k,
    }));
    const group = [
      { artifact: "cmd", order: 4 },
      { artifact: "k1", order: 5, parent: "cmd" },
      { artifact: "k2", order: 6, parent: "cmd" },
      { artifact: "k3", order: 7, parent: "cmd" },
    ];
    const l = layout(
      withKids([...solo, ...group].map((p) => ({ ...p, zone: stage("s0") }))),
    );
    const xs = new Set(
      group.map((g) => node(l, g.artifact).x - node(l, g.artifact).depth * 16),
    );
    expect(xs.size).toBe(1);
    expect(l.stages[0]?.frame.w).toBeGreaterThan(NODE_W * 1.5);
    noOverlap(l);
  });

  it("stacks a group in the toolbelt and the floor", () => {
    for (const zone of [
      { type: "toolbelt" as const },
      { type: "floor" as const },
    ]) {
      const l = layout(
        withKids(
          [
            { artifact: "cmd", zone, order: 0 },
            { artifact: "skill", zone, order: 0, parent: "cmd" },
            { artifact: "next", zone, order: 0 },
          ],
          zone,
        ),
      );
      const cmd = node(l, "cmd");
      const skill = node(l, "skill");
      expect(skill.y).toBe(cmd.y + cmd.h + GAP);
      expect(skill.depth).toBe(1);
      noOverlap(l);
    }
  });

  it("draws a child whose parent is not here as an ordinary node", () => {
    const l = layout(
      withKids([
        { artifact: "skill", zone: stage("s0"), order: 0, parent: "gone" },
      ]),
    );
    expect(node(l, "skill").depth).toBe(0);
    expect(l.links).toEqual([]);
  });

  it("ignores a cycle and a self parent", () => {
    const l = layout(
      withKids([
        { artifact: "a", zone: stage("s0"), order: 0, parent: "b" },
        { artifact: "b", zone: stage("s0"), order: 0, parent: "a" },
        { artifact: "c", zone: stage("s0"), order: 0, parent: "c" },
      ]),
    );
    expect(l.nodes).toHaveLength(3);
    noOverlap(l);
  });
});
