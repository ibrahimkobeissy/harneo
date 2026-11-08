import { mockIPC } from "@tauri-apps/api/mocks";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DrawSummary } from "../../bindings/DrawSummary";
import type { MapDoc } from "../../bindings/MapDoc";
import type { MapEdit } from "../../bindings/MapEdit";
import type { MapView } from "../../bindings/MapView";
import { heavy } from "./fixtures";
import { HarnmapScreen } from ".";

const stageIds = heavy.doc.stages.map((s) => s.id);
// Idea…Ship: prd, spec, sdd, plan, build, inc, fep (pinned), tdd, te, wpp (pinned).
const placements = heavy.doc.placements.slice(0, 10);

function view(over: Partial<MapView> = {}): MapView {
  return {
    drawn: true,
    doc: { ...heavy.doc, placements, changes: [] },
    no: 3,
    createdAt: 1_730_000_000,
    model: "opus",
    durationMs: 52_000,
    description: "Spec first.",
    drawCount: 3,
    facts: heavy.facts,
    bundles: heavy.bundles,
    unplaced: ["rev", "crq"],
    artifacts: heavy.artifacts,
    hooks: heavy.hooks,
    stale: null,
    ...over,
  };
}

const draws: DrawSummary[] = [
  {
    no: 3,
    createdAt: 1_730_000_300,
    model: "opus",
    durationMs: 1000,
    description: "Review is strict.",
    changeLine: "2 artifacts moved",
    restoredFrom: null,
  },
  {
    no: 2,
    createdAt: 1_730_000_200,
    model: "sonnet",
    durationMs: 1000,
    description: "",
    changeLine: "1 placed",
    restoredFrom: null,
  },
  {
    no: 1,
    createdAt: 1_730_000_100,
    model: "opus",
    durationMs: 1000,
    description: "First try.",
    changeLine: "First drawing on the Software development template",
    restoredFrom: null,
  },
];

interface Run {
  edits: MapEdit[];
  calls: { cmd: string; args: unknown }[];
}

/** Applies the parts of an edit these tests look at. */
function apply(v: MapView, edit: MapEdit): MapView {
  const doc: MapDoc = { ...v.doc, placements: [...v.doc.placements] };
  const at = (id: string) => doc.placements.findIndex((p) => p.artifact === id);
  const patch = (id: string, change: object) => {
    const i = at(id);
    const p = doc.placements[i];
    if (p) doc.placements[i] = { ...p, ...change };
  };
  switch (edit.type) {
    case "place":
      doc.placements.push({
        ...heavy.doc.placements[0]!,
        artifact: edit.artifact,
        zone: edit.zone,
        note: edit.note,
        by: "user",
        pinned: true,
      });
      return {
        ...v,
        doc,
        unplaced: v.unplaced.filter((a) => a !== edit.artifact),
      };
    case "move":
      patch(edit.artifact, { zone: edit.zone, pinned: true, by: "user" });
      return { ...v, doc };
    case "detach":
      patch(edit.artifact, { parent: undefined, pinned: true, by: "user" });
      return { ...v, doc };
    case "unpin":
      patch(edit.artifact, { pinned: false });
      return { ...v, doc };
    case "note":
      patch(edit.artifact, { note: edit.text });
      return { ...v, doc };
    case "hide":
      return { ...v, doc: { ...doc, hidden: [...doc.hidden, edit.artifact] } };
    case "show":
      return {
        ...v,
        doc: { ...doc, hidden: doc.hidden.filter((h) => h !== edit.artifact) },
      };
    case "hideType":
      return {
        ...v,
        doc: { ...doc, hiddenTypes: [...doc.hiddenTypes, edit.kind] },
      };
    case "remove":
      return {
        ...v,
        doc: {
          ...doc,
          placements: doc.placements.filter(
            (p) => p.artifact !== edit.artifact,
          ),
        },
        unplaced: [...v.unplaced, edit.artifact],
      };
    case "renameStage":
      return {
        ...v,
        doc: {
          ...doc,
          stages: doc.stages.map((s) =>
            s.id === edit.id ? { ...s, name: edit.name } : s,
          ),
        },
      };
    default:
      return v;
  }
}

/** A toast is also announced, so its text can be on the page twice. */
async function said(text: string) {
  expect((await screen.findAllByText(text)).length).toBeGreaterThan(0);
}

function setup(
  initial: MapView = view(),
  opts: { fail?: boolean; restored?: MapView } = {},
): Run {
  const run: Run = { edits: [], calls: [] };
  let current = initial;
  mockIPC((cmd, args) => {
    run.calls.push({ cmd, args });
    switch (cmd) {
      case "profile_get":
        return { id: 7, name: "Heavy" };
      case "harnmap_get":
        return current;
      case "templates_list":
        return [];
      case "settings_get":
        return null;
      case "library_detail":
        return { artifact: { description: "Does the thing." } };
      case "harnmap_edit": {
        const edit = (args as { edit: MapEdit }).edit;
        if (opts.fail) return Promise.reject("refused");
        run.edits.push(edit);
        current = apply(current, edit);
        return current;
      }
      case "harnmap_history":
        return draws;
      case "harnmap_draw_at":
        return {
          ...initial.doc,
          placements: initial.doc.placements.slice(0, 2),
        };
      case "harnmap_restore":
        current = opts.restored ?? current;
        return 4;
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={["/profiles/7/harnmap"]}>
      <Routes>
        <Route path="profiles/:id/harnmap" element={<HarnmapScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return run;
}

const stage = (id: string) =>
  document.querySelector<HTMLElement>(`[data-stage="${id}"]`) as HTMLElement;
const node = (name: string) =>
  screen.getByRole("button", { name: new RegExp(`^${name},`) });

beforeEach(() => {
  vi.stubGlobal("matchMedia", undefined);
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Harnmap screen: unplaced and placing", () => {
  it("lists the unplaced artifacts by type and filters them", async () => {
    const user = userEvent.setup();
    setup();
    const tree = await screen.findByRole("tree", {
      name: "Unplaced artifacts",
    });
    expect(within(tree).getByText("Commands")).toBeVisible();
    expect(within(tree).getByText("Skills")).toBeVisible();
    await user.type(screen.getByLabelText(/Filter unplaced/), "code");
    expect(within(tree).queryByText("/review")).toBeNull();
    expect(within(tree).getByText("code-review-and-quality")).toBeVisible();
    await user.clear(screen.getByLabelText(/Filter unplaced/));
    await user.type(screen.getByLabelText(/Filter unplaced/), "nothing");
    expect(screen.getByText("Nothing matches “nothing”.")).toBeVisible();
  });

  it("collapses a type group", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByRole("tree", { name: "Unplaced artifacts" });
    await user.click(screen.getByRole("button", { name: /Skills/ }));
    expect(
      screen.queryByRole("treeitem", { name: "Place code-review-and-quality" }),
    ).toBeNull();
  });

  it("drops an unplaced artifact on a stage, asks why and pins it", async () => {
    const user = userEvent.setup();
    const run = setup();
    const item = await screen.findByRole("treeitem", { name: "Place /review" });
    fireEvent.dragStart(item);
    fireEvent.dragOver(stage("test"));
    expect(stage("test")).toHaveAttribute("data-over", "true");
    fireEvent.drop(stage("test"));
    const pop = screen.getByRole("dialog", { name: "Why here?" });
    expect(within(pop).getByLabelText("Where")).toHaveValue("stage:test");
    await user.type(
      within(pop).getByLabelText(/Why here\?/),
      "Perf budget before review",
    );
    await user.click(within(pop).getByRole("button", { name: "Place" }));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({
      type: "place",
      artifact: "rev",
      zone: { type: "stage", id: "test" },
      note: "Perf budget before review",
    });
    await said("/review placed in Test and pinned. It stays on re-draw.");
    expect(screen.queryByRole("dialog", { name: "Why here?" })).toBeNull();
    expect(screen.getByText("Saved just now")).toBeVisible();
  });

  it("drops on the toolbelt and the floor, and Cancel places nothing", async () => {
    const user = userEvent.setup();
    const run = setup();
    const item = await screen.findByRole("treeitem", { name: "Place /review" });
    fireEvent.dragStart(item);
    fireEvent.drop(screen.getByTestId("toolbelt"));
    let pop = screen.getByRole("dialog", { name: "Why here?" });
    await user.click(within(pop).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog", { name: "Why here?" })).toBeNull();
    fireEvent.dragEnd(item);
    fireEvent.dragStart(item);
    fireEvent.drop(screen.getByTestId("floor"));
    pop = screen.getByRole("dialog", { name: "Why here?" });
    await user.keyboard("{Enter}");
    await user.click(within(pop).getByRole("button", { name: "Place" }));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toMatchObject({ zone: { type: "floor" }, note: "" });
    await said("/review placed on the floor and pinned. It stays on re-draw.");
  });

  it("places by choosing the zone when clicked, and Escape closes the popover", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(
      await screen.findByRole("treeitem", { name: "Place /review" }),
    );
    expect(screen.getByRole("dialog", { name: "Why here?" })).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Why here?" })).toBeNull();
    await user.click(screen.getByRole("treeitem", { name: "Place /review" }));
    const pop = screen.getByRole("dialog", { name: "Why here?" });
    await user.selectOptions(within(pop).getByLabelText("Where"), "Toolbelt");
    await user.type(within(pop).getByLabelText(/Why here\?/), "Any stage");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({
      type: "place",
      artifact: "rev",
      zone: { type: "toolbelt" },
      note: "Any stage",
    });
  });

  it("shows a refused edit as a toast and keeps the popover", async () => {
    const user = userEvent.setup();
    setup(view(), { fail: true });
    await user.click(
      await screen.findByRole("treeitem", { name: "Place /review" }),
    );
    await user.click(screen.getByRole("button", { name: "Place" }));
    await said("Could not save that change: refused");
    expect(screen.getByRole("dialog", { name: "Why here?" })).toBeVisible();
  });

  it("summarises the map and puts the cursor in the filter on Place by hand", async () => {
    const user = userEvent.setup();
    setup(
      view({
        stale: {
          added: ["rev"],
          removed: [],
          changed: [],
          categoryChanged: null,
        },
      }),
    );
    const sum = await screen.findByLabelText("On the map");
    expect(
      within(sum).getByText(/10 stations in 7 stages, 4 bundles/),
    ).toBeVisible();
    expect(within(sum).getByText("1 · stay put on re-draw")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Place by hand" }));
    expect(screen.getByLabelText(/Filter unplaced/)).toHaveFocus();
  });
});

describe("Harnmap screen: node detail", () => {
  it("shows why, facts, description and the move, remove and hide actions", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(
      await screen.findByRole("button", { name: /^test-engineer,/ }),
    );
    expect(
      await screen.findByRole("heading", { name: /test-engineer/ }),
    ).toBeVisible();
    expect(screen.getByText("WHY CLAUDE PUT IT HERE")).toBeVisible();
    expect(screen.getByText("test-engineer belongs here.")).toBeVisible();
    expect(
      screen.getByText("Bundled with test-driven-development"),
    ).toBeVisible();
    expect(await screen.findByText("Does the thing.")).toBeVisible();
    expect(screen.getByLabelText("MOVE TO")).toHaveValue("stage:test");

    await user.selectOptions(screen.getByLabelText("MOVE TO"), "Review");
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({
      type: "move",
      artifact: "te",
      zone: { type: "stage", id: "review" },
      order: 0,
    });
    await said(
      "test-engineer moved to Review and pinned. It stays on re-draw.",
    );
    // It is pinned now, and by you.
    expect(screen.getByText("Pinned · stays on re-draw")).toBeVisible();
    expect(screen.getByText("WHY HERE")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Hide" }));
    expect(run.edits[1]).toEqual({ type: "hide", artifact: "te" });
    await said("test-engineer hidden. It is still in the profile.");
    expect(screen.getByText("Hidden on the map")).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Show test-engineer" }),
    );
    expect(run.edits[2]).toEqual({ type: "show", artifact: "te" });
  });

  it("removes a node back to Unplaced", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(
      await screen.findByRole("button", { name: /^test-engineer,/ }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Remove from map" }),
    );
    expect(run.edits[0]).toEqual({ type: "remove", artifact: "te" });
    await said("test-engineer went back to Unplaced");
    expect(
      screen.getByRole("treeitem", { name: "Place test-engineer" }),
    ).toBeVisible();
  });

  it("unpins a pinned node", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(await screen.findByRole("button", { name: /^webperf,/ }));
    await user.click(
      await screen.findByRole("button", { name: "Unpin, let Claude place it" }),
    );
    expect(run.edits[0]).toEqual({ type: "unpin", artifact: "wpp" });
    await said("webperf unpinned. Claude places it on the next re-draw.");
    expect(screen.queryByText("Pinned · stays on re-draw")).toBeNull();
  });

  it("autosaves the note once typing pauses", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(
      await screen.findByRole("button", { name: /^test-engineer,/ }),
    );
    const note = await screen.findByLabelText("NOTE ON THE MAP");
    await user.clear(note);
    await user.type(note, "Gap filler");
    expect(run.edits).toHaveLength(0);
    await waitFor(() => expect(run.edits).toHaveLength(1), { timeout: 2000 });
    expect(run.edits[0]).toEqual({
      type: "note",
      artifact: "te",
      text: "Gap filler",
    });
    expect(screen.getByText("Saved just now")).toBeVisible();
  });

  it("sends a pending note when another node is picked", async () => {
    const user = userEvent.setup();
    const run = setup();
    await user.click(
      await screen.findByRole("button", { name: /^test-engineer,/ }),
    );
    await user.type(await screen.findByLabelText("NOTE ON THE MAP"), "!");
    await user.click(screen.getByRole("button", { name: /^webperf,/ }));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toMatchObject({ type: "note", artifact: "te" });
  });

  it("closes the detail and tells a node without facts or a reason", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: /^webperf,/ }));
    expect(
      await screen.findByText("Nothing special in its files."),
    ).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Close details" }));
    expect(
      screen.getByRole("tree", { name: "Unplaced artifacts" }),
    ).toBeVisible();
  });

  it("moves a placed node by dragging it to another stage or the toolbelt", async () => {
    const run = setup();
    const n = await screen.findByRole("button", { name: /^test-engineer,/ });
    fireEvent.dragStart(n);
    fireEvent.drop(stage("test"));
    expect(run.edits).toHaveLength(0);
    fireEvent.dragStart(n);
    fireEvent.drop(stage("review"));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toMatchObject({
      type: "move",
      artifact: "te",
      zone: { type: "stage", id: "review" },
    });
    // Dropping on another node counts as dropping on its zone.
    fireEvent.dragStart(node("webperf"));
    fireEvent.dragOver(node("write-a-prd"));
    fireEvent.drop(node("write-a-prd"));
    await waitFor(() => expect(run.edits).toHaveLength(2));
    expect(run.edits[1]).toMatchObject({
      artifact: "wpp",
      zone: { type: "stage", id: "idea" },
    });
    fireEvent.dragLeave(stage("idea"));
    fireEvent.dragEnd(node("webperf"));
  });

  it("hides a type with the Types menu and saves it", async () => {
    const user = userEvent.setup();
    const run = setup();
    await screen.findByRole("tree", { name: "Unplaced artifacts" });
    await user.click(screen.getByRole("button", { name: /^Types/ }));
    await user.click(
      screen.getByRole("menuitemcheckbox", { name: /Subagent/ }),
    );
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({ type: "hideType", kind: "agent" });
  });

  it("shows every hidden artifact again with Show all", async () => {
    const user = userEvent.setup();
    const v = view();
    const run = setup({ ...v, doc: { ...v.doc, hidden: ["te", "wpp"] } });
    await user.click(await screen.findByRole("button", { name: "Show all" }));
    await waitFor(() => expect(run.edits).toHaveLength(2));
    expect(run.edits.map((e) => e.type)).toEqual(["show", "show"]);
  });
});

describe("Harnmap screen: stages", () => {
  async function open() {
    const user = userEvent.setup();
    const run = setup();
    await user.click(await screen.findByRole("button", { name: /Stages/ }));
    const pop = screen.getByRole("dialog", { name: "Edit stages" });
    return { user, run, pop };
  }

  it("renames, adds, removes and reorders stages", async () => {
    const { user, run, pop } = await open();
    expect(within(pop).getAllByTestId("stage-row")).toHaveLength(7);
    const name = within(pop).getByLabelText("Stage 2 name");
    await user.clear(name);
    await user.type(name, "Specify{Enter}");
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({
      type: "renameStage",
      id: "spec",
      name: "Specify",
    });
    // An empty name is put back, an unchanged one sends nothing.
    await user.clear(within(pop).getByLabelText("Stage 3 name"));
    await user.tab();
    expect(within(pop).getByLabelText("Stage 3 name")).toHaveValue("Plan");
    expect(run.edits).toHaveLength(1);

    await user.click(within(pop).getByRole("button", { name: "Add stage" }));
    expect(run.edits[1]).toEqual({
      type: "addStage",
      after: "ship",
      name: "New stage",
    });
    await user.click(within(pop).getByRole("button", { name: "Remove Ship" }));
    expect(run.edits[2]).toEqual({ type: "removeStage", id: "ship" });
    await said("Ship removed. Its stations went back to Unplaced.");
    await user.click(within(pop).getByRole("button", { name: "Move Plan up" }));
    expect(run.edits[3]).toEqual({ type: "moveStage", id: "plan", to: 1 });
    await user.click(
      within(pop).getByRole("button", { name: "Move Idea down" }),
    );
    expect(run.edits[4]).toEqual({ type: "moveStage", id: "idea", to: 1 });
    expect(
      within(pop).getByRole("button", { name: "Move Idea up" }),
    ).toBeDisabled();
  });

  it("reorders by drag", async () => {
    const { run, pop } = await open();
    const rows = within(pop).getAllByTestId("stage-row");
    fireEvent.dragStart(rows[4]!);
    fireEvent.dragOver(rows[1]!);
    fireEvent.drop(rows[1]!);
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toEqual({ type: "moveStage", id: "test", to: 1 });
    // Dropping a row on itself does nothing.
    fireEvent.dragStart(rows[2]!);
    fireEvent.drop(rows[2]!);
    expect(run.edits).toHaveLength(1);
  });

  it("closes with Escape and the close button", async () => {
    const { user } = await open();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Edit stages" })).toBeNull();
    await user.click(screen.getByRole("button", { name: /Stages/ }));
    await user.click(screen.getByRole("button", { name: "Close stages" }));
    expect(screen.queryByRole("dialog", { name: "Edit stages" })).toBeNull();
  });
});

describe("Harnmap screen: map history", () => {
  it("lists the draws, previews an old one read-only and goes back", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(
      await screen.findByRole("button", { name: "Map history" }),
    );
    const d3 = await screen.findByRole("group", { name: "Draw 3" });
    expect(within(d3).getByText("Current")).toBeVisible();
    expect(within(d3).getByText("“Review is strict.”")).toBeVisible();
    expect(within(d3).getByText("2 artifacts moved")).toBeVisible();
    const d2 = screen.getByRole("group", { name: "Draw 2" });
    expect(within(d2).getByText(/Sonnet/)).toBeVisible();
    expect(within(d2).queryByText(/“/)).toBeNull();

    await user.click(within(d2).getByRole("button", { name: "Preview" }));
    await said("Previewing draw 2");
    expect(
      screen.queryByRole("button", { name: /^test-engineer,/ }),
    ).toBeNull();
    expect(screen.getByRole("button", { name: /^write-a-prd,/ })).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Back to current" }));
    expect(screen.queryByText("Previewing draw 2")).toBeNull();
    expect(
      screen.getByRole("button", { name: /^test-engineer,/ }),
    ).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Close history" }));
    expect(
      screen.getByRole("tree", { name: "Unplaced artifacts" }),
    ).toBeVisible();
  });

  it("restores an old draw as a new one", async () => {
    const user = userEvent.setup();
    const run = setup(view(), { restored: view({ no: 4, drawCount: 4 }) });
    await user.click(
      await screen.findByRole("button", { name: "Map history" }),
    );
    const d1 = await screen.findByRole("group", { name: "Draw 1" });
    await user.click(within(d1).getByRole("button", { name: "Preview" }));
    const banner = (await screen.findByText("Previewing draw 1")).closest(
      "div",
    ) as HTMLElement;
    await user.click(within(banner).getByRole("button", { name: "Restore" }));
    await said("Draw 1 restored as draw 4. Nothing was overwritten.");
    expect(screen.queryByText("Previewing draw 1")).toBeNull();
    expect(
      run.calls.filter((c) => c.cmd === "harnmap_restore")[0]?.args,
    ).toEqual({
      profileId: 7,
      no: 1,
    });
    // The history was read again for the new draw.
    await waitFor(() =>
      expect(
        run.calls.filter((c) => c.cmd === "harnmap_history").length,
      ).toBeGreaterThan(1),
    );
    await user.click(screen.getByRole("button", { name: "Map history" }));
  });
});

describe("Harnmap screen: list view", () => {
  it("lists placed artifacts in map order, then the unplaced ones", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: "List" }));
    const table = screen.getByRole("table");
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(12);
    const first = within(rows[0]!).getAllByRole("cell");
    expect(first.map((c) => c.textContent)).toEqual([
      "write-a-prd",
      "Line",
      "Idea",
      "Idea into a PRD",
      "Claude",
    ]);
    const pinned = within(rows[9]!).getAllByRole("cell");
    expect(pinned[4]).toHaveTextContent("You (pinned)");
    const unplaced = within(rows[10]!).getAllByRole("cell");
    expect(unplaced.map((c) => c.textContent)).toEqual([
      "/review",
      "Unplaced",
      "—",
      "—",
      "—",
    ]);
  });

  it("shows toolbelt rows, you without a pin, and selects a name on the map", async () => {
    const user = userEvent.setup();
    const v = view();
    const extra = [
      { ...heavy.doc.placements[16]! },
      {
        ...heavy.doc.placements[0]!,
        artifact: "prd",
        by: "user" as const,
        pinned: false,
      },
    ];
    setup({
      ...v,
      doc: {
        ...v.doc,
        placements: [extra[0]!, ...v.doc.placements.slice(1, 3)],
      },
    });
    await user.click(await screen.findByRole("button", { name: "List" }));
    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(within(rows[2]!).getAllByRole("cell")[1]).toHaveTextContent(
      "Toolbelt",
    );
    expect(within(rows[2]!).getAllByRole("cell")[2]).toHaveTextContent("—");
    await user.click(
      screen.getByRole("button", { name: "spec-driven-development" }),
    );
    expect(
      await screen.findByRole("heading", { name: /spec-driven-development/ }),
    ).toBeVisible();
    expect(screen.queryByRole("table")).toBeNull();
    expect(stageIds).toContain("spec");
  });

  it("an unplaced name goes back to the map", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: "List" }));
    await user.click(screen.getByRole("button", { name: "/review" }));
    expect(screen.queryByRole("table")).toBeNull();
    expect(
      screen.getByRole("tree", { name: "Unplaced artifacts" }),
    ).toBeVisible();
  });

  it("a user placement without a pin reads You", async () => {
    const user = userEvent.setup();
    const v = view();
    const p = { ...v.doc.placements[0]!, by: "user" as const, pinned: false };
    setup({ ...v, doc: { ...v.doc, placements: [p] } });
    await user.click(await screen.findByRole("button", { name: "List" }));
    const row = within(screen.getByRole("table")).getAllByRole("row")[1]!;
    expect(within(row).getAllByRole("cell")[4]).toHaveTextContent("You");
    expect(within(row).getAllByRole("cell")[4]).not.toHaveTextContent("pinned");
  });
});

/** The spec stage with /spec running spec-driven-development, also run by /plan. */
function withChild(): MapView {
  const v = view();
  return {
    ...v,
    doc: {
      ...v.doc,
      placements: v.doc.placements.map((p) =>
        p.artifact === "sdd"
          ? { ...p, parent: "spec", alsoRunBy: ["plan"] }
          : p,
      ),
    },
    facts: {
      ...v.facts,
      sdd: [{ kind: { type: "modelOnly" }, text: "Model only: not invocable" }],
    },
  };
}

describe("Harnmap screen: parent and child", () => {
  it("draws the child narrower under its parent, marks a model-only skill and explains both", async () => {
    const user = userEvent.setup();
    setup(withChild());
    const child = await screen.findByRole("button", {
      name: /^spec-driven-development,/,
    });
    expect(child).toHaveAttribute("data-depth", "1");
    expect(child).toHaveAccessibleName(/model only/);
    expect(screen.getAllByTestId("child-link")).toHaveLength(1);
    await user.click(
      screen.getByRole("button", {
        name: "How spec-driven-development takes part in the flow",
      }),
    );
    const tip = screen.getByRole("dialog", { name: /spec-driven-development/ });
    expect(within(tip).getByText(/Runs only through \/spec/)).toBeVisible();
    expect(within(tip).getByText("/plan")).toBeVisible();
    expect(within(tip).getByText("Model only: not invocable")).toBeVisible();
  });

  it("detaches from the node detail", async () => {
    const user = userEvent.setup();
    const run = setup(withChild());
    await user.click(
      await screen.findByRole("button", { name: /^spec-driven-development,/ }),
    );
    expect(await screen.findByText("Runs only through /spec.")).toBeVisible();
    expect(screen.getByText("Also run by /plan.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Detach" }));
    expect(run.edits[0]).toEqual({ type: "detach", artifact: "sdd" });
    await said(
      "spec-driven-development detached from /spec and pinned. It stays on re-draw.",
    );
    await waitFor(() =>
      expect(screen.queryAllByTestId("child-link")).toHaveLength(0),
    );
  });

  it("shows no Detach for an ordinary node", async () => {
    const user = userEvent.setup();
    setup(withChild());
    await user.click(await screen.findByRole("button", { name: /^\/spec,/ }));
    await screen.findByText("NOTE ON THE MAP");
    expect(screen.queryByRole("button", { name: "Detach" })).toBeNull();
  });

  it("detaches by dragging the child to another stage, and moves the parent alone", async () => {
    const run = setup(withChild());
    fireEvent.dragStart(
      await screen.findByRole("button", { name: /^spec-driven-development,/ }),
    );
    fireEvent.drop(stage("test"));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toMatchObject({
      type: "move",
      artifact: "sdd",
      zone: { type: "stage", id: "test" },
    });
    await said(
      "spec-driven-development detached from /spec, moved to Test and pinned. It stays on re-draw.",
    );
  });

  it("moves the parent with one edit", async () => {
    const run = setup(withChild());
    fireEvent.dragStart(
      await screen.findByRole("button", { name: /^\/spec,/ }),
    );
    fireEvent.drop(stage("test"));
    await waitFor(() => expect(run.edits).toHaveLength(1));
    expect(run.edits[0]).toMatchObject({ type: "move", artifact: "spec" });
  });

  it("indents the child under its parent in the list view", async () => {
    const user = userEvent.setup();
    setup(withChild());
    await user.click(await screen.findByRole("button", { name: "List" }));
    const row = screen
      .getByRole("button", { name: "spec-driven-development" })
      .closest("td");
    expect(row).toHaveAttribute("data-depth", "1");
    expect(row).toHaveTextContent("runs only through /spec");
  });
});
