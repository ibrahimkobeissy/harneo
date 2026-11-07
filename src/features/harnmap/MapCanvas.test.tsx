import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fixtures, heavy } from "./fixtures";
import { MapCanvas } from "./MapCanvas";
import { sourceOf } from "./tooltip";
import {
  clampZoom,
  DEFAULT_VIEW,
  type MapData,
  type MapViewState,
} from "./types";

function Harness({
  data = heavy,
  initial = DEFAULT_VIEW,
  onSelect = () => {},
}: {
  data?: MapData;
  initial?: MapViewState;
  onSelect?: (id: string | null) => void;
}) {
  const [view, setView] = useState(initial);
  const [selected, setSelected] = useState<string | null>(null);
  return (
    <MapCanvas
      data={data}
      view={view}
      selected={selected}
      onView={setView}
      onSelect={(id) => {
        setSelected(id);
        onSelect(id);
      }}
    />
  );
}

const world = () =>
  screen.getByTestId("viewport").firstElementChild as HTMLElement;

describe("the four shapes", () => {
  it.each([
    ["line", 7, 22],
    ["parallel", 7, 9],
    ["cycle", 7, 7],
    ["diamond", 7, 7],
    ["big", 7, 47],
  ] as const)("draws %s with its stages and nodes", (name, stages, nodes) => {
    const { container } = render(<Harness data={fixtures[name]} />);
    expect(screen.getAllByTestId("stage")).toHaveLength(stages);
    expect(container.querySelectorAll("[data-node]")).toHaveLength(nodes);
    expect(screen.getByTestId("toolbelt")).toBeInTheDocument();
    expect(screen.getByTestId("floor")).toBeInTheDocument();
  });

  it("shows stage number, name, count and light", () => {
    render(<Harness />);
    const stage = screen.getAllByTestId("stage")[3];
    if (!stage) throw new Error("no stage");
    expect(within(stage).getByText("04")).toBeInTheDocument();
    expect(within(stage).getByText("Build")).toBeInTheDocument();
    expect(within(stage).getByText("3")).toBeInTheDocument();
    expect(within(stage).getByTestId("light")).toHaveAttribute(
      "data-on",
      "true",
    );
  });

  it("lights only the stages that have stations", () => {
    const empty = {
      ...fixtures.cycle,
      doc: { ...fixtures.cycle.doc, placements: [] },
    };
    render(<Harness data={empty} />);
    expect(
      screen.getAllByTestId("light").some((l) => l.dataset.on === "false"),
    ).toBe(true);
  });

  it("draws loops as dashed arcs labelled as Claude's reading, the hook rail and bundles", () => {
    render(<Harness />);
    expect(
      screen.getByText(/changes requested · Claude’s reading/),
    ).toBeInTheDocument();
    expect(screen.getByTestId("hook-rail")).toHaveTextContent(
      "hook · PreToolUse · Write|Edit",
    );
    expect(screen.getAllByTestId("bundle")).toHaveLength(4);
    expect(screen.getByLabelText("Legend")).toBeInTheDocument();
  });

  it("draws the cycle's return arrow, the diamonds and the parallel tracks", () => {
    const { container: c1 } = render(<Harness data={fixtures.cycle} />);
    expect(c1.querySelectorAll("path").length).toBeGreaterThan(3);
    const { container: c2 } = render(<Harness data={fixtures.diamond} />);
    expect(c2.querySelectorAll("path").length).toBeGreaterThan(3);
    render(<Harness data={fixtures.parallel} />);
    expect(screen.getByText("Pre-production")).toBeInTheDocument();
  });

  it("shows the pin mark and the note", () => {
    render(<Harness />);
    const node = screen.getByRole("button", { name: /webperf, Skill, pinned/ });
    expect(
      within(node).getByRole("img", { name: "pinned" }),
    ).toBeInTheDocument();
    expect(within(node).getByText("Pinned by you")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "/plan, Command" }),
    ).toBeInTheDocument();
  });

  it("skips a placement whose artifact is unknown and omits the rail without hooks", () => {
    const data: MapData = {
      ...heavy,
      artifacts: { ...heavy.artifacts },
      hooks: [],
    };
    delete data.artifacts["spec"];
    render(<Harness data={data} />);
    expect(screen.queryByRole("button", { name: /\/spec,/ })).toBeNull();
    expect(screen.queryByTestId("hook-rail")).toBeNull();
  });
});

describe("selection", () => {
  it("selects a node on click and clears on a background click", async () => {
    const onSelect = vi.fn();
    render(<Harness onSelect={onSelect} />);
    const node = screen.getByRole("button", { name: "/plan, Command" });
    await userEvent.click(node);
    expect(onSelect).toHaveBeenLastCalledWith("plan");
    expect(node).toHaveAttribute("aria-pressed", "true");
    const viewport = screen.getByTestId("viewport");
    fireEvent.pointerDown(viewport, { clientX: 5, clientY: 5 });
    fireEvent.pointerUp(viewport);
    expect(onSelect).toHaveBeenLastCalledWith(null);
  });
});

describe("the heartbeat", () => {
  const beat = (name: string) =>
    screen.getByRole("button", { name: `How ${name} takes part in the flow` });

  it("opens a tooltip on click, blue while open, and closes on a second click", async () => {
    render(<Harness />);
    const hb = beat("/plan");
    expect(hb).toHaveAttribute("tabindex", "-1");
    expect(hb).toHaveAttribute("data-open", "false");
    await userEvent.click(hb);
    const tip = screen.getByRole("dialog");
    expect(tip).toHaveTextContent("Role");
    expect(tip).toHaveTextContent("When");
    expect(tip).toHaveTextContent("Hands off");
    expect(hb).toHaveAttribute("data-open", "true");
    expect(hb).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(hb);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("keeps only one tooltip open", async () => {
    render(<Harness />);
    await userEvent.click(beat("/plan"));
    await userEvent.click(beat("/build"));
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog")).toHaveAccessibleName(/\/build/);
    expect(beat("/plan")).toHaveAttribute("data-open", "false");
  });

  it("tags each line files or Claude", async () => {
    render(<Harness />);
    await userEvent.click(beat("test-engineer"));
    const tip = screen.getByRole("dialog");
    const tags = within(tip)
      .getAllByText(/^(files|Claude)$/)
      .map((t) => t.textContent);
    // Role: Claude. When: no fact. Hands off: a bundle fact.
    expect(tags).toEqual(["Claude", "Claude", "files"]);
    await userEvent.click(beat("/spec"));
    const manual = within(screen.getByRole("dialog"))
      .getAllByText(/^(files|Claude)$/)
      .map((t) => t.textContent);
    expect(manual).toEqual(["Claude", "files", "Claude"]);
  });

  it("opens with i on a focused node, moves focus in, and Escape closes it and refocuses", async () => {
    render(<Harness />);
    const node = screen.getByRole("button", { name: "/plan, Command" });
    node.focus();
    await userEvent.keyboard("i");
    const tip = screen.getByRole("dialog");
    expect(tip).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(node).toHaveFocus();
  });

  it("ignores other keys and modified i", async () => {
    render(<Harness />);
    const node = screen.getByRole("button", { name: "/plan, Command" });
    node.focus();
    await userEvent.keyboard("x");
    await userEvent.keyboard("{Control>}i{/Control}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("closes on a background click and ignores Escape when nothing is open", async () => {
    render(<Harness />);
    await userEvent.keyboard("{Escape}");
    await userEvent.click(beat("/plan"));
    const viewport = screen.getByTestId("viewport");
    fireEvent.pointerDown(viewport, { clientX: 5, clientY: 5 });
    fireEvent.pointerUp(viewport);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("flips the tooltip to the left near the right edge", async () => {
    render(<Harness />);
    await userEvent.click(beat("release"));
    const left = parseFloat(screen.getByRole("dialog").style.left);
    const node = screen.getByRole("button", { name: "release, Workflow" });
    expect(left).toBeLessThan(parseFloat(node.style.left));
  });

  it("says when a line was not stated", async () => {
    const data: MapData = {
      ...heavy,
      doc: {
        ...heavy.doc,
        placements: heavy.doc.placements.map((p) => ({ ...p, when: "" })),
      },
    };
    render(<Harness data={data} />);
    await userEvent.click(beat("/plan"));
    expect(screen.getByRole("dialog")).toHaveTextContent("Not stated");
  });

  it("gives every dot the same beat, from the shared clock", () => {
    // The page clock moves a whole number of beats while the dots mount, so every dot must land
    // on the same phase. A fixed clock keeps the test independent of machine load.
    let tick = 0;
    const now = vi
      .spyOn(performance, "now")
      .mockImplementation(() => 1234 + 2000 * tick++);
    try {
      render(<Harness />);
      const dots = screen
        .getAllByRole("button", { name: /takes part in the flow/ })
        .map((b) => (b.firstElementChild as HTMLElement).style.animationDelay);
      expect(dots.length).toBeGreaterThan(1);
      expect(new Set(dots)).toEqual(new Set(["-1234ms"]));
    } finally {
      now.mockRestore();
    }
  });
});

describe("tooltip source tags", () => {
  const f = (type: string) => [{ kind: { type }, text: "x" }] as never;
  it("tags role as Claude and backs the rest by facts", () => {
    expect(sourceOf("role", f("bundle"))).toBe("claude");
    expect(sourceOf("handsOff", f("bundle"))).toBe("files");
    expect(sourceOf("handsOff", f("skills"))).toBe("files");
    expect(sourceOf("handsOff", f("allowedMcp"))).toBe("files");
    expect(sourceOf("handsOff", f("manualOnly"))).toBe("claude");
    for (const t of [
      "manualOnly",
      "rulePaths",
      "hook",
      "alwaysLoaded",
      "outputStyle",
    ]) {
      expect(sourceOf("when", f(t))).toBe("files");
    }
    expect(sourceOf("when", f("bundle"))).toBe("claude");
    expect(sourceOf("when", [])).toBe("claude");
  });
});

describe("zoom, pan and fit", () => {
  const pct = () => screen.getByText(/^\d+%$/).textContent;

  it("zooms with the buttons in 10 % steps within 40 to 150 %", async () => {
    render(<Harness />);
    expect(pct()).toBe("100%");
    await userEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(pct()).toBe("110%");
    expect(world().style.transform).toContain("scale(1.1)");
    await userEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    await userEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(pct()).toBe("90%");
    for (let i = 0; i < 8; i++)
      await userEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(pct()).toBe("40%");
    expect(screen.getByRole("button", { name: "Zoom out" })).toBeDisabled();
    for (let i = 0; i < 11; i++)
      await userEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(pct()).toBe("150%");
    expect(screen.getByRole("button", { name: "Zoom in" })).toBeDisabled();
  });

  it("clamps the zoom", () => {
    expect(clampZoom(0.1)).toBe(0.4);
    expect(clampZoom(3)).toBe(1.5);
    expect(clampZoom(0.8)).toBe(0.8);
  });

  it("zooms with Ctrl+wheel around the pointer and ignores a plain wheel", () => {
    render(<Harness />);
    const viewport = screen.getByTestId("viewport");
    fireEvent.wheel(viewport, { deltaY: -100 });
    expect(pct()).toBe("100%");
    fireEvent.wheel(viewport, {
      deltaY: -100,
      ctrlKey: true,
      clientX: 100,
      clientY: 50,
    });
    expect(pct()).toBe("110%");
    expect(world().style.transform).toBe("translate(-10px, -5px) scale(1.1)");
    fireEvent.wheel(viewport, {
      deltaY: 100,
      ctrlKey: true,
      clientX: 100,
      clientY: 50,
    });
    expect(pct()).toBe("100%");
  });

  it("stops at the limits with the wheel", () => {
    render(<Harness initial={{ ...DEFAULT_VIEW, zoom: 1.5 }} />);
    fireEvent.wheel(screen.getByTestId("viewport"), {
      deltaY: -100,
      ctrlKey: true,
    });
    expect(pct()).toBe("150%");
  });

  it("pans by dragging the background but not by dragging a node", () => {
    render(<Harness />);
    const viewport = screen.getByTestId("viewport");
    fireEvent.pointerMove(viewport, { clientX: 50, clientY: 50 });
    fireEvent.pointerDown(viewport, { clientX: 10, clientY: 10 });
    fireEvent.pointerMove(viewport, { clientX: 60, clientY: 40 });
    fireEvent.pointerUp(viewport);
    expect(world().style.transform).toBe("translate(50px, 30px) scale(1)");
    const node = screen.getByRole("button", { name: "/plan, Command" });
    fireEvent.pointerDown(node, { clientX: 0, clientY: 0 });
    fireEvent.pointerMove(viewport, { clientX: 99, clientY: 99 });
    expect(world().style.transform).toBe("translate(50px, 30px) scale(1)");
  });

  it("fits the map to the container", async () => {
    render(<Harness />);
    const viewport = screen.getByTestId("viewport");
    vi.spyOn(viewport, "clientWidth", "get").mockReturnValue(600);
    vi.spyOn(viewport, "clientHeight", "get").mockReturnValue(400);
    await userEvent.click(screen.getByRole("button", { name: /fit/i }));
    const zoom = Number(/scale\(([\d.]+)\)/.exec(world().style.transform)?.[1]);
    expect(zoom).toBeGreaterThanOrEqual(0.4);
    expect(zoom).toBeLessThan(1);
    expect(pct()).toBe(`${Math.round(zoom * 100)}%`);
  });

  it("fits with a default size when the container is not measured", async () => {
    render(<Harness />);
    await userEvent.click(screen.getByRole("button", { name: /fit/i }));
    expect(world().style.transform).toContain("translate(");
  });

  it("turns into plain boxes below 75 %", () => {
    const { rerender } = render(<Harness />);
    expect(screen.getByTestId("viewport")).toHaveAttribute(
      "data-quiet",
      "false",
    );
    rerender(<Harness key="low" initial={{ ...DEFAULT_VIEW, zoom: 0.7 }} />);
    expect(screen.getByTestId("viewport")).toHaveAttribute(
      "data-quiet",
      "true",
    );
  });
});

describe("layers, types and hidden artifacts", () => {
  it("hides the line, the toolbelt and the floor with their nodes", async () => {
    render(<Harness />);
    await userEvent.click(screen.getByRole("button", { name: "Line" }));
    expect(screen.queryAllByTestId("stage")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: /\/plan,/ })).toBeNull();
    expect(screen.getByRole("button", { name: /github,/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Toolbelt" }));
    expect(screen.queryByTestId("toolbelt")).toBeNull();
    expect(screen.queryByRole("button", { name: /github,/ })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Floor" }));
    expect(screen.queryByTestId("floor")).toBeNull();
    expect(screen.queryByRole("button", { name: /CLAUDE.md,/ })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Line" }));
    expect(screen.getAllByTestId("stage")).toHaveLength(7);
  });

  it("lists the types with counts and hides a type's nodes", async () => {
    render(<Harness />);
    await userEvent.click(screen.getByRole("button", { name: "Types" }));
    const menu = screen.getByRole("menu", { name: "Types" });
    const agents = within(menu).getByRole("menuitemcheckbox", {
      name: /Subagent/,
    });
    expect(agents).toHaveTextContent("3");
    expect(agents).toHaveAttribute("aria-checked", "true");
    await userEvent.click(agents);
    expect(agents).toHaveAttribute("aria-checked", "false");
    expect(screen.queryByRole("button", { name: /code-reviewer,/ })).toBeNull();
    expect(
      screen.getByRole("button", { name: /Types \(1 hidden\)/ }),
    ).toBeInTheDocument();
    await userEvent.click(agents);
    expect(
      screen.getByRole("button", { name: /code-reviewer,/ }),
    ).toBeInTheDocument();
    await userEvent.keyboard("x");
    fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("hides the artifacts the view lists and closes the gap", () => {
    render(<Harness initial={{ ...DEFAULT_VIEW, hidden: ["rev", "crq"] }} />);
    expect(screen.queryByRole("button", { name: /\/review,/ })).toBeNull();
    expect(
      screen.queryByRole("button", { name: /code-review-and-quality,/ }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: /code-reviewer,/ }),
    ).toBeInTheDocument();
  });
});

describe("animation", () => {
  it("pauses while the window is hidden and resumes when it shows", () => {
    const { container } = render(<Harness />);
    const root = container.firstElementChild as HTMLElement;
    expect(root).toHaveAttribute("data-paused", "false");
    const state = vi.spyOn(document, "visibilityState", "get");
    state.mockReturnValue("hidden");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(root).toHaveAttribute("data-paused", "true");
    state.mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(root).toHaveAttribute("data-paused", "false");
    state.mockRestore();
  });

  it("starts paused when the window is already hidden", () => {
    const state = vi
      .spyOn(document, "visibilityState", "get")
      .mockReturnValue("hidden");
    const { container } = render(<Harness />);
    expect(container.firstElementChild).toHaveAttribute("data-paused", "true");
    state.mockRestore();
  });
});
