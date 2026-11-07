import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import type { MapView } from "../../bindings/MapView";
import type { Placement } from "../../bindings/Placement";
import { HarnmapCard } from "./HarnmapCard";

const placement = (artifact: string): Placement => ({
  artifact,
  zone: { type: "floor" },
  order: 0,
  note: "",
  why: "",
  role: "",
  when: "",
  handsOff: "",
  by: "claude",
  pinned: false,
});

function view(over: Partial<MapView> = {}): MapView {
  return {
    drawn: true,
    doc: {
      category: "software",
      shape: "line",
      stages: [
        { id: "a", name: "A", origin: "template" },
        { id: "b", name: "B", origin: "template" },
        { id: "c", name: "C", origin: "template" },
      ],
      loops: [],
      phases: [],
      parallel: [],
      placements: [placement("one"), placement("gone")],
      hidden: [],
      hiddenTypes: [],
      changes: [],
    },
    no: 3,
    createdAt: 1_730_000_000,
    model: "opus",
    durationMs: 1000,
    description: "",
    drawCount: 3,
    facts: {},
    bundles: [],
    unplaced: ["two"],
    artifacts: {
      one: { name: "one", kind: "skill" },
      two: { name: "two", kind: "skill" },
    },
    hooks: [],
    stale: null,
    ...over,
  };
}

function show(result: () => unknown) {
  mockIPC((cmd) => (cmd === "harnmap_get" ? result() : undefined));
  render(
    <MemoryRouter>
      <HarnmapCard profile={4n} version={1} />
    </MemoryRouter>,
  );
}

describe("Harnmap card", () => {
  it("shows an up to date map: badge, draw line and the way in", async () => {
    show(() => view());
    expect(await screen.findByText("Up to date")).toHaveAttribute(
      "data-tone",
      "ok",
    );
    expect(
      screen.getByText(/3 drawings in history · 1 of 2 artifacts placed/),
    ).toBeVisible();
    expect(screen.getByText(/^Drawn /)).toBeVisible();
    expect(screen.getByRole("link", { name: "Open Harnmap" })).toHaveAttribute(
      "href",
      "/profiles/4/harnmap",
    );
    expect(
      screen.getByRole("link", { name: "Open the Harnmap" }),
    ).toBeVisible();
    expect(
      screen.getByTestId("mini-map").querySelectorAll("rect"),
    ).toHaveLength(3);
  });

  it("says out of date when the profile changed since the draw", async () => {
    show(() =>
      view({
        drawCount: 1,
        stale: {
          added: ["two"],
          removed: [],
          changed: [],
          categoryChanged: null,
        },
      }),
    );
    expect(await screen.findByText("Out of date")).toHaveAttribute(
      "data-tone",
      "warn",
    );
    expect(screen.getByText(/1 drawing in history/)).toBeVisible();
  });

  it("says not drawn yet before the first draw", async () => {
    show(() => view({ drawn: false, drawCount: 0, no: 0 }));
    expect(await screen.findByText("Not drawn yet")).toHaveAttribute(
      "data-tone",
      "neutral",
    );
    expect(screen.getByText(/Draw it to see this profile/)).toBeVisible();
  });

  it("stays out of the way when the map can't be read", async () => {
    let asked = false;
    show(() => {
      asked = true;
      throw new Error("down");
    });
    await waitFor(() => expect(asked).toBe(true));
    expect(screen.queryByRole("heading", { name: "Harnmap" })).toBeNull();
  });

  it("stays out of the way when nothing comes back", async () => {
    let asked = false;
    show(() => {
      asked = true;
      return undefined;
    });
    await waitFor(() => expect(asked).toBe(true));
    expect(screen.queryByRole("heading", { name: "Harnmap" })).toBeNull();
  });
});
