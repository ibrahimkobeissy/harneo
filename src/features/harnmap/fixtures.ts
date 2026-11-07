import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Fact } from "../../bindings/Fact";
import type { MapDoc } from "../../bindings/MapDoc";
import type { Placement } from "../../bindings/Placement";
import type { Shape } from "../../bindings/Shape";
import type { Zone } from "../../bindings/Zone";
import type { MapData } from "./types";

/** Fixture maps for tests and later screens: the four shapes and a 47-artifact map. */

type Spec = [
  id: string,
  kind: ArtifactKind,
  name: string,
  zone: string,
  note: string,
];

function zoneOf(z: string): Zone {
  if (z === "toolbelt") return { type: "toolbelt" };
  if (z === "floor") return { type: "floor" };
  return { type: "stage", id: z };
}

function stages(names: string[]): MapDoc["stages"] {
  return names.map((name) => ({
    id: name.toLowerCase().replace(/[^a-z]+/g, "-"),
    name,
    origin: "template" as const,
  }));
}

function build(
  shape: Shape,
  category: MapDoc["category"],
  stageNames: string[],
  specs: Spec[],
  extra: Partial<MapDoc> = {},
  facts: Record<string, Fact[]> = {},
  bundles: string[][] = [],
): MapData {
  const counts = new Map<string, number>();
  const placements: Placement[] = specs.map(([id, , name, zone, note]) => {
    const order = counts.get(zone) ?? 0;
    counts.set(zone, order + 1);
    return {
      artifact: id,
      zone: zoneOf(zone),
      order,
      note,
      why: `${name} belongs here.`,
      role: `Does the ${name} work.`,
      when: `When ${zone} starts.`,
      handsOff: "Hands the result to the next stage.",
      by: id.endsWith("pp") ? "user" : "claude",
      pinned: id.endsWith("pp"),
    };
  });
  return {
    doc: {
      category,
      shape,
      stages: stages(stageNames),
      loops: [],
      phases: [],
      parallel: [],
      placements,
      hidden: [],
      hiddenTypes: [],
      changes: [],
      ...extra,
    },
    artifacts: Object.fromEntries(
      specs.map(([id, kind, name]) => [id, { name, kind }]),
    ),
    facts,
    bundles,
    hooks: [{ event: "PreToolUse", matcher: "Write|Edit" }],
  };
}

const SETTINGS_HEAVY: Spec[] = [
  ["claude", "claudeMd", "CLAUDE.md", "floor", "Loaded in every session"],
  ["rules", "rule", "no-unwrap", "floor", "Rust style"],
  ["settings", "settings", "settings.json", "floor", "Permissions"],
  ["style", "outputStyle", "terse", "floor", "Short replies"],
];

/** The Heavy profile (software, line): seven stages, a Review loop, bundles, pins. */
export const heavy: MapData = build(
  "line",
  "software",
  ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"],
  [
    ["prd", "skill", "write-a-prd", "idea", "Idea into a PRD"],
    ["spec", "command", "/spec", "spec", "Opens the spec"],
    ["sdd", "skill", "spec-driven-development", "spec", "Spec before code"],
    ["plan", "command", "/plan", "plan", "Spec into tasks"],
    ["build", "command", "/build", "build", "Runs the next task"],
    ["inc", "skill", "incremental-implementation", "build", "Thin slices"],
    ["fep", "skill", "frontend-ui-engineering", "build", "Pinned by you"],
    ["tdd", "skill", "test-driven-development", "test", "Failing test first"],
    ["te", "agent", "test-engineer", "test", "Fills coverage gaps"],
    ["wpp", "skill", "webperf", "test", "Pinned by you"],
    ["rev", "command", "/review", "review", "Five-axis review"],
    ["crq", "skill", "code-review-and-quality", "review", "The checklist"],
    ["cr", "agent", "code-reviewer", "review", "Reads the diff cold"],
    ["sa", "agent", "security-auditor", "review", "Auth and input code"],
    ["ship", "command", "/ship", "ship", "Pre-launch checklist"],
    ["rel", "workflow", "release", "ship", "Tag, check, publish"],
    ["gh", "mcp", "github", "toolbelt", "Issues and PRs"],
    ["ctx", "mcp", "context7", "toolbelt", "Library docs"],
    ...SETTINGS_HEAVY,
  ],
  { loops: [{ from: "review", to: "build", label: "changes requested" }] },
  {
    te: [
      {
        kind: { type: "bundle" },
        text: "Bundled with test-driven-development",
      },
    ],
    spec: [
      { kind: { type: "manualOnly" }, text: "Runs only when you type it" },
    ],
    claude: [
      { kind: { type: "alwaysLoaded" }, text: "Loaded in every session" },
    ],
    cr: [{ kind: { type: "skills" }, text: "Uses code-review-and-quality" }],
  },
  [
    ["spec", "sdd"],
    ["build", "inc", "fep"],
    ["tdd", "te", "wpp"],
    ["rev", "crq", "cr"],
  ],
);

/** The Video profile (parallel): Edit and Sound & captions as two tracks, three phases. */
export const video: MapData = build(
  "parallel",
  "video",
  [
    "Concept",
    "Script",
    "Storyboard",
    "Shoot & assets",
    "Edit",
    "Sound & captions",
    "Publish",
  ],
  [
    ["hook", "skill", "hook-writer", "concept", "Opening lines"],
    ["scr", "command", "/script", "script", "Beat sheet to script"],
    ["sb", "skill", "storyboard", "storyboard", "Shot list"],
    ["shot", "skill", "shot-planner", "shoot-assets", "Coverage plan"],
    ["cut", "agent", "rough-cut", "edit", "First assembly"],
    ["cap", "skill", "captions", "sound-captions", "Burn-in subtitles"],
    ["pub", "workflow", "publish", "publish", "Upload and tag"],
    ["ff", "mcp", "ffmpeg", "toolbelt", "Transcode"],
    ["brand", "claudeMd", "CLAUDE.md", "floor", "Channel voice"],
  ],
  {
    parallel: ["edit", "sound-captions"],
    phases: [
      { name: "Pre-production", stages: ["concept", "script", "storyboard"] },
      { name: "Production", stages: ["shoot-assets"] },
      {
        name: "Post-production",
        stages: ["edit", "sound-captions", "publish"],
      },
    ],
  },
);

/** A social profile on the cycle shape, the return arrow closing it. */
export const cycle: MapData = build(
  "cycle",
  "social",
  ["Plan", "Ideate", "Create", "Adapt", "Schedule", "Engage", "Measure"],
  [
    ["cal", "skill", "content-calendar", "plan", "Weekly slots"],
    ["idea", "skill", "ideation", "ideate", "Angles"],
    ["post", "command", "/post", "create", "Draft a post"],
    ["adp", "agent", "adapter", "adapt", "Per platform"],
    ["sch", "workflow", "schedule", "schedule", "Queue it"],
    ["eng", "skill", "reply-guide", "engage", "Answer comments"],
    ["met", "skill", "metrics", "measure", "Weekly review"],
  ],
  { loops: [{ from: "measure", to: "plan", label: "next week" }] },
);

/** A design profile on the diamond shape. */
export const diamond: MapData = build(
  "diamond",
  "design",
  [
    "Brief",
    "Research",
    "Define",
    "Concepts",
    "Prototype",
    "Critique",
    "Hand-off",
  ],
  [
    ["brf", "skill", "brief-check", "brief", "Gaps in the ask"],
    ["res", "skill", "user-research", "research", "Interviews"],
    ["def", "skill", "problem-framing", "define", "One sentence"],
    ["con", "skill", "concepts", "concepts", "Three directions"],
    ["pro", "command", "/prototype", "prototype", "Clickable draft"],
    ["cri", "agent", "critic", "critique", "Cold read"],
    ["hnd", "workflow", "handoff", "hand-off", "Specs for devs"],
  ],
  { loops: [{ from: "critique", to: "prototype", label: "revise" }] },
);

/** A 47-artifact map on the line shape, for the smoothness check. */
export const big: MapData = (() => {
  const names = ["Idea", "Spec", "Plan", "Build", "Test", "Review", "Ship"];
  const kindsList: ArtifactKind[] = ["skill", "command", "agent", "workflow"];
  const specs: Spec[] = Array.from({ length: 47 }, (_, i) => {
    const where =
      i < 36 ? names[i % names.length] : i < 43 ? "toolbelt" : "floor";
    const zone = (where ?? "idea").toLowerCase();
    return [
      `n${i}`,
      i >= 43 ? "rule" : (kindsList[i % kindsList.length] ?? "skill"),
      `artifact-number-${i}`,
      zone,
      `Note ${i}`,
    ];
  });
  return build("line", "software", names, specs, {
    loops: [{ from: "review", to: "build", label: "changes requested" }],
  });
})();

export const fixtures = { line: heavy, parallel: video, cycle, diamond, big };
