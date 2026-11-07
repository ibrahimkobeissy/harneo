import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { MapView } from "../../bindings/MapView";
import type { Staleness } from "../../bindings/Staleness";
import type { Template } from "../../bindings/Template";
import type { TemplateChange } from "../../bindings/TemplateChange";
import { asFailure, changeLines, clock, replayGap, took } from "./draw";
import { heavy } from "./fixtures";
import { HarnmapScreen } from ".";

const templates: Template[] = [
  {
    id: "software",
    label: "Software development",
    shape: "line",
    stages: [],
    loops: [],
    phases: [],
    parallel: [],
    suggestions: [
      "Spec first, always.",
      "TDD lives inside Build.",
      "Review sends work back to Build.",
    ],
  },
  {
    id: "research",
    label: "Research",
    shape: "line",
    stages: [],
    loops: [],
    phases: [],
    parallel: [],
    suggestions: ["a", "b", "c"],
  },
];

const placements = heavy.doc.placements.slice(0, 3);
const changes: TemplateChange[] = [
  { type: "stageKept", id: "idea" },
  { type: "stageKept", id: "spec" },
  { type: "stageRenamed", id: "plan", from: "Plan", to: "Planning" },
  { type: "stageAdded", id: "qa", name: "QA" },
  { type: "stageRemoved", id: "ship", name: "Ship" },
  { type: "loopAdded", from: "review", to: "build", label: "again" },
  { type: "unplaced", count: 1 },
];

function view(over: Partial<MapView> = {}): MapView {
  return {
    drawn: true,
    doc: { ...heavy.doc, placements, changes },
    no: 2,
    createdAt: 1_730_000_000,
    model: "opus",
    durationMs: 52_000,
    description: "Spec first.",
    drawCount: 2,
    facts: heavy.facts,
    bundles: heavy.bundles,
    unplaced: ["rev"],
    artifacts: heavy.artifacts,
    hooks: heavy.hooks,
    stale: null,
    ...over,
  };
}

function skeleton(): MapView {
  return view({
    drawn: false,
    no: 0,
    doc: { ...heavy.doc, placements: [], changes: [] },
    description: "",
    drawCount: 0,
    unplaced: Object.keys(heavy.artifacts),
  });
}

interface Setup {
  calls: { cmd: string; args: unknown }[];
  /** Resolve or reject the pending draw. */
  finish: (v: "ok" | DrawFailure) => void;
}

function setup(
  initial: MapView,
  opts: { stored?: unknown; after?: MapView; instant?: DrawFailure } = {},
): Setup {
  const calls: Setup["calls"] = [];
  let release: (v: "ok" | DrawFailure) => void = () => undefined;
  let drawn = false;
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "profile_get":
        return { id: 7, name: "Heavy" };
      case "harnmap_get":
        return drawn ? (opts.after ?? view({ no: 3 })) : initial;
      case "templates_list":
        return templates;
      case "settings_get":
        return opts.stored ?? null;
      case "claude_cli_status":
        return { found: true, path: "/bin/claude", version: "2" };
      case "harnmap_draw":
        if (opts.instant) return Promise.reject(opts.instant);
        return new Promise((resolve, reject) => {
          release = (v) => {
            if (v === "ok") {
              drawn = true;
              resolve({ no: 3, model: "opus", durationMs: 61_000, changes });
            } else reject(v);
          };
        });
      case "harnmap_cancel":
        release({ type: "cancelled" });
        return undefined;
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={["/profiles/7/harnmap"]}>
      <Routes>
        <Route path="profiles/:id/harnmap" element={<HarnmapScreen />} />
        <Route path="profiles/:id" element={<p>Builder</p>} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls, finish: (v) => release(v) };
}

const has = (s: Setup, cmd: string) => s.calls.filter((c) => c.cmd === cmd);

beforeEach(() => {
  vi.stubGlobal("matchMedia", undefined);
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Harnmap screen: first draw", () => {
  it("shows the skeleton, chips and facts, waits, replays and shows the result", async () => {
    const user = userEvent.setup();
    const s = setup(skeleton());
    expect(
      await screen.findByRole("heading", { name: "Draw the factory" }),
    ).toBeVisible();
    expect(screen.getByText("HARNMAP · NOT DRAWN YET")).toBeVisible();
    expect(screen.getByText("Software development")).toBeVisible();
    const facts = screen.getByLabelText("What a draw involves");
    expect(within(facts).getByText("22 artifacts")).toBeVisible();
    expect(within(facts).getByText("your plan · Opus")).toBeVisible();
    expect(within(facts).getByText("usually 1–3 min")).toBeVisible();
    expect(screen.getByText(/What is sent:/)).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "+ TDD lives inside Build." }),
    );
    const box = screen.getByLabelText("How do you see this factory?");
    expect(box).toHaveValue("TDD lives inside Build.");
    await user.click(
      screen.getByRole("button", { name: "+ Spec first, always." }),
    );
    expect(box).toHaveValue("TDD lives inside Build. Spec first, always.");

    await user.click(screen.getByRole("button", { name: /^Draw/ }));
    expect(await screen.findByRole("timer")).toHaveTextContent("0:00");
    expect(screen.getByText(/CLAUDE IS WORKING · OPUS/)).toBeVisible();
    expect(has(s, "harnmap_draw")[0]?.args).toEqual({
      profileId: 7,
      description: "TDD lives inside Build. Spec first, always.",
    });

    await act(async () => s.finish("ok"));
    expect(
      await screen.findByText("DRAW 3 · SAVED", {}, { timeout: 4000 }),
    ).toBeVisible();
    expect(screen.getByText(/Opus · 1 min 1 s · 3 of 22 placed/)).toBeVisible();
    const list = screen.getByRole("list", {
      name: "Changes from the template",
    });
    expect(
      within(list).getByText(/Kept 2 stages\. Renamed 1, added 1\./),
    ).toBeVisible();
    expect(within(list).getByText("Renamed Plan to Planning.")).toBeVisible();
    expect(within(list).getByText("Added the stage QA.")).toBeVisible();
    expect(within(list).getByText("Removed the stage Ship.")).toBeVisible();
    expect(
      within(list).getByText(/The loop from Review to Build \(again\)/),
    ).toBeVisible();
    expect(within(list).getByText(/Left 1 artifact unplaced/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Open the map" }));
    expect(
      screen.getByText(/DRAW 3 · 2 IN HISTORY|DRAW 2 · 2 IN HISTORY/),
    ).toBeVisible();
  });

  it("Ctrl+Enter draws", async () => {
    const user = userEvent.setup();
    const s = setup(skeleton());
    await screen.findByRole("heading", { name: "Draw the factory" });
    await user.click(screen.getByLabelText("How do you see this factory?"));
    await user.keyboard("{Control>}{Enter}{/Control}");
    await waitFor(() => expect(has(s, "harnmap_draw")).toHaveLength(1));
  });

  it("Cancel stops the draw, saves nothing and keeps the description", async () => {
    const user = userEvent.setup();
    const s = setup(skeleton());
    await screen.findByRole("heading", { name: "Draw the factory" });
    await user.type(
      screen.getByLabelText("How do you see this factory?"),
      "Mine",
    );
    await user.click(screen.getByRole("button", { name: /^Draw/ }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(has(s, "harnmap_cancel")).toHaveLength(1);
    expect(
      await screen.findByRole("heading", { name: "Draw the factory" }),
    ).toBeVisible();
    expect(screen.getByLabelText("How do you see this factory?")).toHaveValue(
      "Mine",
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows the result at once when the system asks for no motion", async () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    const user = userEvent.setup();
    const s = setup(skeleton());
    await screen.findByRole("heading", { name: "Draw the factory" });
    await user.click(screen.getByRole("button", { name: /^Draw/ }));
    await act(async () => s.finish("ok"));
    expect(await screen.findByText("DRAW 3 · SAVED")).toBeVisible();
  });

  it("cannot draw an empty profile", async () => {
    setup(view({ drawn: false, no: 0, artifacts: {}, unplaced: [] }));
    expect(await screen.findByRole("button", { name: /^Draw/ })).toBeDisabled();
    expect(
      screen.getByRole("link", { name: "Add artifacts from the library" }),
    ).toHaveAttribute("href", "/profiles/7");
  });
});

describe("Harnmap screen: drawn map and re-draw", () => {
  it("shows the header, keeps the map while re-drawing and adds a draw", async () => {
    const user = userEvent.setup();
    const s = setup(view());
    expect(await screen.findByText(/Draw 2 · .* · Opus · 52 s/)).toBeVisible();
    expect(screen.getByText("Saved")).toBeVisible();
    expect(screen.getByRole("button", { name: "List" })).toBeDisabled();
    expect(screen.getByRole("link", { name: "Heavy" })).toHaveAttribute(
      "href",
      "/profiles/7",
    );
    expect(screen.getByRole("link", { name: "Close" })).toHaveAttribute(
      "href",
      "/profiles/7",
    );

    await user.click(screen.getByRole("button", { name: "Re-draw" }));
    const dialog = screen.getByRole("dialog", { name: "Re-draw the map" });
    expect(
      within(dialog).getByLabelText("How do you see this factory?"),
    ).toHaveValue("Spec first.");
    expect(within(dialog).getByText("Stays put: 0 pinned")).toBeVisible();
    await user.clear(
      within(dialog).getByLabelText("How do you see this factory?"),
    );
    await user.type(
      within(dialog).getByLabelText("How do you see this factory?"),
      "Tighter",
    );
    await user.click(
      within(dialog).getByRole("button", { name: /Draw again/ }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(await screen.findByRole("timer")).toBeVisible();
    // The current map stays visible while Claude works.
    expect(screen.getAllByText("write-a-prd").length).toBeGreaterThan(0);
    expect(has(s, "harnmap_draw")[0]?.args).toEqual({
      profileId: 7,
      description: "Tighter",
    });
    await act(async () => s.finish("ok"));
    expect(
      await screen.findByText("DRAW 3 · SAVED", {}, { timeout: 4000 }),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Open the map" }));
    expect(await screen.findByText(/Draw 3 · /)).toBeVisible();
  });

  it("lists the pinned artifacts and closes with Escape and Cancel", async () => {
    const user = userEvent.setup();
    const doc = {
      ...heavy.doc,
      changes: [],
      placements: placements.map((p, i) => ({ ...p, pinned: i === 0 })),
    };
    setup(view({ doc }));
    await user.click(await screen.findByRole("button", { name: "Re-draw" }));
    const pins = screen.getByRole("list", { name: "Pinned artifacts" });
    expect(within(pins).getByText("write-a-prd")).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Re-draw" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Re-draw" }));
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(await screen.findByRole("timer")).toBeVisible();
  });

  it("says so when nothing is on the line", async () => {
    const doc = {
      ...heavy.doc,
      changes: [],
      placements: heavy.doc.placements.slice(-3),
    };
    setup(view({ doc }));
    expect(
      await screen.findByRole("region", { name: "Nothing on the line" }),
    ).toBeVisible();
  });

  it("shows a map the app could not load as gone", async () => {
    mockIPC(() => Promise.reject("boom"));
    render(
      <MemoryRouter initialEntries={["/profiles/7/harnmap"]}>
        <Routes>
          <Route path="profiles/:id/harnmap" element={<HarnmapScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(
      await screen.findByText("This profile is no longer here"),
    ).toBeVisible();
  });
});

describe("Harnmap screen: failures", () => {
  const card = (name: string) => screen.findByRole("region", { name });

  async function fail(
    failure: DrawFailure,
    initial = skeleton(),
    stored?: unknown,
  ) {
    const user = userEvent.setup();
    const s = setup(initial, { stored });
    await screen.findByRole("button", { name: /^(Draw|Re-draw)/ });
    await user.click(
      screen.queryByRole("button", { name: "Re-draw" }) ??
        screen.getByRole("button", { name: /^Draw/ }),
    );
    if (initial.drawn)
      await user.click(screen.getByRole("button", { name: /Draw again/ }));
    await screen.findByRole("timer");
    await act(async () => s.finish(failure));
    return { user, s };
  }

  it("claude missing: Check again and the setup guide", async () => {
    const { user, s } = await fail({ type: "claudeMissing" });
    const c = within(await card("claude CLI not found"));
    await user.click(
      c.getByRole("button", { name: "Claude Code setup guide" }),
    );
    await waitFor(() =>
      expect(has(s, "open_link")[0]?.args).toEqual({ name: "claudeInstall" }),
    );
    await user.click(c.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(has(s, "claude_cli_status")).toHaveLength(1));
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: "claude CLI not found" }),
      ).toBeNull(),
    );
  });

  it("not signed in: Check again", async () => {
    const { user } = await fail({
      type: "claudeFailed",
      kind: "not_logged_in",
      message: "x",
    });
    const c = within(await card("claude not signed in"));
    await user.click(c.getByRole("button", { name: "Check again" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("region", { name: "claude not signed in" }),
      ).toBeNull(),
    );
  });

  it("check again keeps the card when the CLI is still missing", async () => {
    const { user, s } = await fail({ type: "claudeMissing" });
    void s;
    mockIPC((cmd) =>
      cmd === "claude_cli_status"
        ? { found: false, path: null, version: null }
        : undefined,
    );
    await user.click(
      within(await card("claude CLI not found")).getByRole("button", {
        name: "Check again",
      }),
    );
    expect(await card("claude CLI not found")).toBeVisible();
  });

  it("usage limit: message and Try again", async () => {
    const { user, s } = await fail({
      type: "claudeFailed",
      kind: "limit",
      message: "x",
    });
    const c = within(await card("Usage limit"));
    expect(c.getByText(/usage limit reached/)).toBeVisible();
    await user.click(c.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(has(s, "harnmap_draw")).toHaveLength(2));
  });

  it("model unavailable: Draw with Sonnet stores the setting and draws again", async () => {
    const { user, s } = await fail({
      type: "claudeFailed",
      kind: "model_unavailable",
      message: "x",
    });
    const c = within(await card("Model not available"));
    expect(c.getByText(/Opus isn’t available/)).toBeVisible();
    await user.click(c.getByRole("button", { name: "Draw with Sonnet" }));
    await waitFor(() => expect(has(s, "harnmap_draw")).toHaveLength(2));
    expect(has(s, "settings_set")[0]?.args).toEqual({
      key: "harnmap.model",
      value: "sonnet",
    });
    expect(await screen.findByText(/CLAUDE IS WORKING · SONNET/)).toBeVisible();
  });

  it("model unavailable on Sonnet offers Opus, and Cancel dismisses", async () => {
    const { user } = await fail(
      { type: "claudeFailed", kind: "model_unavailable", message: "x" },
      skeleton(),
      "sonnet",
    );
    const c = within(await card("Model not available"));
    expect(c.getByRole("button", { name: "Draw with Opus" })).toBeVisible();
    await user.click(c.getByRole("button", { name: "Cancel" }));
    expect(
      screen.queryByRole("region", { name: "Model not available" }),
    ).toBeNull();
  });

  it("timeout: Use Sonnet and Try again", async () => {
    const { user, s } = await fail({
      type: "claudeFailed",
      kind: "timeout",
      message: "x",
    });
    const c = within(await card("Timed out"));
    expect(c.getByText(/No answer after 5 min/)).toBeVisible();
    await user.click(c.getByRole("button", { name: "Use Sonnet" }));
    await waitFor(() => expect(has(s, "harnmap_draw")).toHaveLength(2));
    expect(has(s, "settings_set")[0]?.args).toEqual({
      key: "harnmap.model",
      value: "sonnet",
    });
  });

  it("timeout on Sonnet has no Use Sonnet", async () => {
    await fail(
      { type: "claudeFailed", kind: "timeout", message: "x" },
      skeleton(),
      "sonnet",
    );
    const c = within(await card("Timed out"));
    expect(c.queryByRole("button", { name: "Use Sonnet" })).toBeNull();
    expect(c.getByRole("button", { name: "Try again" })).toBeVisible();
  });

  it("answer did not check out: Show the answer lists the reasons", async () => {
    const { user } = await fail({
      type: "badAnswer",
      reasons: ["placed /ship twice", "unknown stage"],
    });
    const c = within(await card("Answer did not check out"));
    await user.click(c.getByRole("button", { name: "Show the answer" }));
    expect(c.getByText("placed /ship twice")).toBeVisible();
    await user.click(c.getByRole("button", { name: "Hide the answer" }));
    expect(c.queryByText("placed /ship twice")).toBeNull();
    await user.click(c.getByRole("button", { name: "Try again" }));
  });

  it("another failure shows its message; the map stays when drawn", async () => {
    await fail({ type: "other", message: "disk full" }, view());
    const c = within(await card("Drawing failed"));
    expect(c.getByText(/disk full/)).toBeVisible();
    expect(c.getByText("Your map is unchanged.")).toBeVisible();
    expect(screen.getAllByText("write-a-prd").length).toBeGreaterThan(0);
  });

  it("an unknown claude failure kind shows its message", async () => {
    await fail({ type: "claudeFailed", kind: "other", message: "weird" });
    expect(
      within(await card("Drawing failed")).getByText(/weird/),
    ).toBeVisible();
  });

  it("empty profile: the card links to the builder and dismisses", async () => {
    const { user } = await fail({ type: "emptyProfile" });
    const c = within(await card("Empty profile"));
    expect(c.getByRole("link", { name: "Add from library" })).toHaveAttribute(
      "href",
      "/profiles/7",
    );
    await user.click(c.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("region", { name: "Empty profile" })).toBeNull();
  });

  it("a rejection that is not a tagged value becomes an other failure", async () => {
    const s = setup(skeleton(), { instant: undefined });
    void s;
    expect(asFailure("plain")).toEqual({ type: "other", message: "plain" });
    expect(asFailure({ type: "cancelled" })).toEqual({ type: "cancelled" });
  });
});

describe("Harnmap screen: out of date", () => {
  const stale = (over: Partial<Staleness> = {}): Staleness => ({
    added: ["wpp"],
    removed: ["gone"],
    changed: ["tdd"],
    categoryChanged: null,
    ...over,
  });

  it("lists + − ~ and offers Re-draw", async () => {
    const user = userEvent.setup();
    setup(view({ stale: stale() }));
    const c = within(
      await screen.findByRole("region", { name: "Map out of date" }),
    );
    const rows = c.getByRole("list", { name: "What changed" });
    expect(within(rows).getByText("webperf")).toBeVisible();
    expect(within(rows).getByText("in Unplaced")).toBeVisible();
    expect(within(rows).getByText("gone")).toBeVisible();
    expect(within(rows).getByText("removed from the map")).toBeVisible();
    expect(within(rows).getByText("test-driven-development")).toBeVisible();
    expect(within(rows).getByText("differs from the drawn copy")).toBeVisible();
    await user.click(c.getByRole("button", { name: "Re-draw" }));
    expect(screen.getByRole("dialog")).toBeVisible();
  });

  it("category changed: Re-draw on the new template, or keep the old map", async () => {
    const user = userEvent.setup();
    setup(
      view({
        stale: stale({
          added: [],
          removed: [],
          changed: [],
          categoryChanged: { from: "software", to: "research" },
        }),
      }),
    );
    const c = within(
      await screen.findByRole("region", { name: "Category changed" }),
    );
    expect(c.getByText("Research")).toBeVisible();
    await user.click(c.getByRole("button", { name: "Re-draw on Research" }));
    expect(screen.getByRole("dialog")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(
      c.getByRole("button", { name: "Keep the old map for now" }),
    );
    expect(
      screen.queryByRole("region", { name: "Category changed" }),
    ).toBeNull();
  });
});

describe("draw helpers", () => {
  it("formats the clock and the duration", () => {
    expect(clock(65_000)).toBe("1:05");
    expect(clock(-5)).toBe("0:00");
    expect(took(52_000)).toBe("52 s");
    expect(took(72_000)).toBe("1 min 12 s");
  });

  it("squeezes the replay gap so that it ends in about 3 s", () => {
    expect(replayGap(10)).toBe(110);
    expect(replayGap(60)).toBe(50);
    expect(replayGap(0)).toBe(110);
  });

  it("writes the changes in plain words", () => {
    const lines = changeLines(
      [
        { type: "stageKept", id: "a" },
        { type: "unplaced", count: 0 },
      ],
      (id) => id,
      [],
    );
    expect(lines).toEqual([
      { mark: "=", text: "Kept 1 stage. Renamed none, added none." },
    ]);
    const some = changeLines([{ type: "unplaced", count: 2 }], (id) => id, [
      "x",
      "y",
    ]);
    expect(some[1]?.names).toEqual(["x", "y"]);
  });
});
