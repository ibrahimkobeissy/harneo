import { mockIPC } from "@tauri-apps/api/mocks";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DrawFailure } from "../../bindings/DrawFailure";
import type { FitSummary } from "../../bindings/FitSummary";
import type { FitVerdict } from "../../bindings/FitVerdict";
import type { FitView } from "../../bindings/FitView";
import type { MapView } from "../../bindings/MapView";
import { heavy } from "./fixtures";
import { HarnfitScreen } from ".";
import { ghostBox } from "./ghost";
import { layout } from "./layout";
import {
  candidateFor,
  fitProblem,
  harnfitPath,
  verdictLabel,
  verdictTone,
} from "./harnfit";

function mapView(drawn = true): MapView {
  return {
    drawn,
    doc: heavy.doc,
    no: 1,
    createdAt: 1_730_000_000,
    model: "opus",
    durationMs: 52_000,
    description: "",
    drawCount: 1,
    facts: heavy.facts,
    bundles: heavy.bundles,
    unplaced: [],
    artifacts: heavy.artifacts,
    hooks: heavy.hooks,
    stale: null,
  };
}

function verdict(over: Partial<FitVerdict> = {}): FitVerdict {
  return {
    verdict: "conflicts",
    zone: { type: "stage", id: "test" },
    why: "Clashes with the TDD skill.",
    change: null,
    conflictsWith: ["tdd"],
    quotes: [
      {
        artifact: "candidate",
        file: "SKILL.md",
        line: 12,
        text: "Write the code first.",
      },
      {
        artifact: "tdd",
        file: "SKILL.md",
        line: 8,
        text: "Never write code before a test.",
      },
    ],
    droppedQuotes: 0,
    missingMcp: [],
    candidate: {
      key: "lib:lite",
      label: "tdd-lite",
      kind: "skill",
      source: "Library",
    },
    model: "opus",
    durationMs: 48_000,
    createdAt: 1_730_000_000,
    ...over,
  };
}

function fitView(over: Partial<FitVerdict> = {}, stale = false): FitView {
  return {
    verdict: verdict(over),
    stale,
    changes: {
      added: ["gh"],
      removed: [],
      changed: [],
      candidateChanged: true,
    },
  };
}

interface Opts {
  drawn?: boolean;
  saved?: FitView | null;
  ran?: FitView;
  fail?: DrawFailure;
  addError?: string;
  summaries?: FitSummary[];
}

function Where() {
  const at = useLocation();
  return (
    <p data-testid="where">
      {at.pathname}|{JSON.stringify(at.state)}
    </p>
  );
}

function setup(path: string, opts: Opts = {}, state?: unknown) {
  const calls: { cmd: string; args: unknown }[] = [];
  let release: (v: FitView | DrawFailure) => void = () => undefined;
  let saved: FitView | null = null;
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "profile_get":
        return { id: 7, name: "Heavy" };
      case "harnmap_get":
        return mapView(opts.drawn ?? true);
      case "harnfit_list":
        return opts.summaries ?? [];
      case "harnfit_get":
        return opts.saved ?? saved;
      case "settings_get":
        return null;
      case "harnfit_run":
        return new Promise((resolve, reject) => {
          release = (v) => {
            if ("verdict" in v) {
              saved = v;
              resolve(v);
            } else reject(v);
          };
          if (opts.fail) reject(opts.fail);
          else if (opts.ran) resolve(opts.ran);
        });
      case "harnfit_cancel":
        release({ type: "cancelled" });
        return undefined;
      case "harnfit_add":
        return opts.addError
          ? Promise.reject(opts.addError)
          : (opts.saved ?? fitView());
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[{ pathname: path, state }]}>
      <Routes>
        <Route
          path="profiles/:id/harnfit/:candidate"
          element={<HarnfitScreen />}
        />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls, finish: (v: FitView | DrawFailure) => release(v) };
}

const ran = (s: ReturnType<typeof setup>, cmd: string) =>
  s.calls.filter((c) => c.cmd === cmd);

beforeEach(() => {
  vi.stubGlobal("matchMedia", undefined);
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Harnfit helpers", () => {
  it("labels, colours and parses", () => {
    expect(verdictLabel("fits")).toBe("Fits");
    expect(verdictLabel("fitsWithChanges")).toBe("Fits with changes");
    expect(verdictLabel("conflicts")).toBe("Conflicts");
    expect(verdictTone("fits")).toBe("ok");
    expect(verdictTone("fitsWithChanges")).toBe("warn");
    expect(verdictTone("conflicts")).toBe("bad");
    expect(harnfitPath(7, "lib:a b")).toBe("/profiles/7/harnfit/lib%3Aa%20b");
    expect(candidateFor("lib:x", null)).toEqual({ type: "library", id: "x" });
    expect(candidateFor("src:x", null)).toBeNull();
    const preview = { type: "preview", token: "t", candidateId: "c" } as const;
    expect(candidateFor("preview:c", { candidate: preview })).toBe(preview);
    expect(fitProblem("the profile has no map yet: draw it first")).toBe(
      "noMap",
    );
    expect(fitProblem("Harnfit does not take a plugin whole yet")).toBe(
      "plugin",
    );
    expect(fitProblem("import it into the library first")).toBe("importFirst");
    expect(fitProblem("x is already in the profile")).toBe("alreadyIn");
    expect(fitProblem("there is no saved fit for this candidate")).toBe(
      "notFound",
    );
    expect(fitProblem("boom")).toBeNull();
  });

  it("puts the ghost after the zone's last node", () => {
    const input = {
      shape: heavy.doc.shape,
      stages: heavy.doc.stages,
      loops: heavy.doc.loops,
      phases: heavy.doc.phases,
      parallel: heavy.doc.parallel,
      placements: heavy.doc.placements,
      bundles: heavy.bundles,
      hasHooks: true,
    };
    const lay = layout(input);
    const test = ghostBox(lay, { type: "stage", id: "test" });
    const last = lay.nodes.filter(
      (n) => n.zone.type === "stage" && n.zone.id === "test",
    );
    expect(test.y).toBeGreaterThan(Math.max(...last.map((n) => n.y)));
    const empty = layout({ ...input, placements: [] });
    expect(ghostBox(empty, { type: "toolbelt" }).y).toBeGreaterThan(
      empty.toolbelt.y,
    );
    expect(ghostBox(empty, { type: "floor" }).y).toBeGreaterThan(empty.floor.y);
    const belt = lay.nodes.filter((n) => n.zone.type === "toolbelt");
    const after = ghostBox(lay, { type: "toolbelt" });
    expect(after.x).toBeGreaterThan(Math.min(...belt.map((n) => n.x)));
    // A band that is full wraps to the next row.
    const wraps = Array.from({ length: 30 }, (_, n) => {
      const full = layout({
        ...input,
        placements: Array.from({ length: n + 1 }, (_, i) => ({
          artifact: `m${i}`,
          zone: { type: "floor" as const },
          order: i,
        })),
      });
      return { box: ghostBox(full, { type: "floor" }), band: full.floor };
    });
    expect(
      wraps.some(({ box, band }) => box.x === band.x + 8 && box.y > band.y + 8),
    ).toBe(true);
  });
});

describe("Harnfit screen", () => {
  it("opens a saved fit at once, without a Claude call, with every part of the verdict", async () => {
    const s = setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView(
        {
          change: "Limit it to spikes.",
          droppedQuotes: 1,
          missingMcp: [
            { server: "postgres", inLibrary: false },
            { server: "github", inLibrary: true },
          ],
        },
        true,
      ),
      summaries: [
        {
          candidate: {
            key: "lib:lite",
            label: "tdd-lite",
            kind: "skill",
            source: "Library",
          },
          verdict: "conflicts",
          createdAt: 1_730_000_000,
          stale: true,
        },
      ],
    });
    const card = await screen.findByRole("region", {
      name: "Verdict for tdd-lite",
    });
    expect(ran(s, "harnfit_run")).toHaveLength(0);
    expect(card).toHaveAttribute("data-verdict", "conflicts");
    expect(within(card).getByText("Conflicts")).toBeVisible();
    expect(within(card).getByText("Clashes with the TDD skill.")).toBeVisible();
    expect(within(card).getByText(/tdd-lite · SKILL.md:12/)).toBeVisible();
    expect(
      within(card).getByText(/test-driven-development · SKILL.md:8/),
    ).toBeVisible();
    expect(
      within(card).getByText(
        /Both quotes checked word for word against the files\. Line numbers found by Harneo, not Claude\./,
      ),
    ).toBeVisible();
    expect(
      within(card).getByText(/1 quote couldn’t be checked and was removed\./),
    ).toBeVisible();
    expect(within(card).getByText(/Limit it to spikes\./)).toBeVisible();
    expect(
      within(card).getByText("isn’t in your library.", { exact: false }),
    ).toBeVisible();
    expect(
      within(card).getByText(/is in your library\. Add still puts only/),
    ).toBeVisible();
    expect(
      within(card).getByRole("button", { name: "Import postgres first" }),
    ).toBeVisible();
    expect(
      within(card).getByRole("button", { name: "Add without it" }),
    ).toBeVisible();
    expect(screen.getAllByTestId("conflict-line")).toHaveLength(1);
    expect(screen.getByTestId("ghost")).toHaveTextContent("NEW");
    const saved = screen.getByRole("list", { name: "Saved fits" });
    expect(within(saved).getByText(/out of date/)).toBeVisible();
    // The stale banner offers Run again and Keep.
    const banner = screen.getByText("This fit is out of date.")
      .parentElement as HTMLElement;
    await userEvent
      .setup()
      .click(
        within(banner).getByRole("button", { name: "Keep the old verdict" }),
      );
    expect(screen.queryByText("This fit is out of date.")).toBeNull();
  });

  it("colours fits green and fits with changes amber", async () => {
    setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView({ verdict: "fits", conflictsWith: [], quotes: [] }),
    });
    const card = await screen.findByRole("region", {
      name: "Verdict for tdd-lite",
    });
    expect(card).toHaveAttribute("data-verdict", "fits");
    expect(
      within(card).getByText("Fits").closest("[data-tone]"),
    ).toHaveAttribute("data-tone", "ok");
    expect(screen.queryByTestId("conflict-line")).toBeNull();
    expect(
      within(card).getByRole("button", { name: "Add to profile" }),
    ).toBeEnabled();
  });

  it("runs with the honest wait when there is no saved fit, then shows the verdict", async () => {
    const s = setup("/profiles/7/harnfit/lib%3Alite", {}, { name: "tdd-lite" });
    expect(await screen.findByRole("timer")).toHaveTextContent("0:00");
    expect(screen.getByText(/CLAUDE IS WORKING · OPUS/)).toBeVisible();
    expect(screen.getByText(/usually takes 30–90 s/)).toBeVisible();
    expect(ran(s, "harnfit_run")[0]?.args).toEqual({
      profileId: 7,
      candidate: { type: "library", id: "lite" },
    });
    await act(async () => s.finish(fitView()));
    expect(
      await screen.findByRole("region", { name: "Verdict for tdd-lite" }),
    ).toBeVisible();
  });

  it("cancels the wait and saves nothing", async () => {
    const s = setup("/profiles/7/harnfit/lib%3Alite");
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(ran(s, "harnfit_cancel")).toHaveLength(1));
    await waitFor(() => expect(screen.queryByRole("timer")).toBeNull());
  });

  it("keeps the old verdict while Run again works", async () => {
    const user = userEvent.setup();
    const s = setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView({ why: "Old why." }),
    });
    await screen.findByText("Old why.");
    await user.click(screen.getByRole("button", { name: "Run again" }));
    expect(await screen.findByRole("timer")).toBeVisible();
    expect(screen.getByText("Old why.")).toBeVisible();
    expect(
      screen.getByText(/The saved verdict stays until the new one lands/),
    ).toBeVisible();
    await act(async () => s.finish(fitView({ why: "New why." })));
    expect(await screen.findByText("New why.")).toBeVisible();
    expect(screen.queryByText("Old why.")).toBeNull();
  });

  it("runs a preview candidate from the route state and moves to its saved address", async () => {
    const s = setup(
      "/profiles/7/harnfit/preview%3Ac1",
      {},
      {
        candidate: { type: "preview", token: "tok", candidateId: "c1" },
        name: "tdd-lite",
      },
    );
    await screen.findByRole("timer");
    expect(ran(s, "harnfit_run")[0]?.args).toEqual({
      profileId: 7,
      candidate: { type: "preview", token: "tok", candidateId: "c1" },
    });
    await act(async () =>
      s.finish(
        fitView({
          candidate: {
            key: "src:s@c:skill/lite",
            label: "tdd-lite",
            kind: "skill",
            source: "s",
          },
        }),
      ),
    );
    const card = await screen.findByRole("region", {
      name: "Verdict for tdd-lite",
    });
    // Not in the library yet: Add says so and is off.
    expect(
      within(card).getByRole("button", { name: "Add to profile" }),
    ).toBeDisabled();
    expect(within(card).getByText(/Import it first/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Run again" })).toBeEnabled();
  });

  it("disables Run again for a preview candidate whose preview is gone", async () => {
    const s = setup("/profiles/7/harnfit/src%3As%40c%3Askill%2Flite", {
      saved: fitView({
        candidate: {
          key: "src:s@c:skill/lite",
          label: "tdd-lite",
          kind: "skill",
          source: "s",
        },
      }),
    });
    await screen.findByRole("region", { name: "Verdict for tdd-lite" });
    expect(screen.getByRole("button", { name: "Run again" })).toBeDisabled();
    expect(screen.getByText(/needs the import preview/)).toBeVisible();
    expect(ran(s, "harnfit_run")).toHaveLength(0);
  });

  it("says there is no saved fit for a preview key without its preview", async () => {
    setup("/profiles/7/harnfit/src%3Ax");
    expect(
      await screen.findByRole("region", { name: "No saved fit" }),
    ).toBeVisible();
  });

  it("adds to the profile and returns to the map with the toast", async () => {
    const user = userEvent.setup();
    const s = setup("/profiles/7/harnfit/lib%3Alite", { saved: fitView() });
    await user.click(
      await screen.findByRole("button", { name: "Add to profile" }),
    );
    await waitFor(() => expect(screen.getByTestId("where")).toBeVisible());
    expect(ran(s, "harnfit_add")[0]?.args).toEqual({
      profileId: 7,
      candidateKey: "lib:lite",
    });
    expect(ran(s, "harnmap_edit")).toHaveLength(0);
    const where = screen.getByTestId("where").textContent ?? "";
    expect(where).toContain("/profiles/7/harnmap");
    expect(where).toContain(
      "tdd-lite added to Heavy and placed on the map, pinned.",
    );
  });

  it("places it where the node was dropped", async () => {
    const user = userEvent.setup();
    const s = setup("/profiles/7/harnfit/lib%3Alite", { saved: fitView() });
    await user.click(
      await screen.findByRole("button", { name: "Place elsewhere" }),
    );
    expect(screen.getAllByText(/Drag the dashed node/).length).toBeGreaterThan(
      0,
    );
    fireEvent.dragStart(screen.getByTestId("ghost"));
    fireEvent.dragOver(screen.getByTestId("floor"));
    fireEvent.drop(screen.getByTestId("floor"));
    expect(screen.getByText(/Would go: Floor/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Add to profile" }));
    await waitFor(() => expect(ran(s, "harnmap_edit")).toHaveLength(1));
    expect(ran(s, "harnmap_edit")[0]?.args).toMatchObject({
      profileId: 7,
      edit: { type: "place", artifact: "lite", zone: { type: "floor" } },
    });
  });

  it("says why an add was refused", async () => {
    const user = userEvent.setup();
    setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView(),
      addError: "import it into the library first",
    });
    await user.click(
      await screen.findByRole("button", { name: "Add to profile" }),
    );
    expect(
      (await screen.findAllByText(/Import it first: it isn’t in your library/))
        .length,
    ).toBeGreaterThan(0);
  });

  it("shows the already-in-profile card when the add says so", async () => {
    const user = userEvent.setup();
    setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView(),
      addError: "tdd-lite is already in the profile",
    });
    await user.click(
      await screen.findByRole("button", { name: "Add to profile" }),
    );
    expect(
      await screen.findByRole("region", { name: "Already in the profile" }),
    ).toBeVisible();
  });

  it("discards back to where the user came from, leaving the fit saved", async () => {
    const user = userEvent.setup();
    setup(
      "/profiles/7/harnfit/lib%3Alite",
      { saved: fitView() },
      { from: "/library" },
    );
    await user.click(await screen.findByRole("button", { name: "Discard" }));
    const where = (await screen.findByTestId("where")).textContent ?? "";
    expect(where).toContain("/library");
    expect(where).toContain("Discarded. The fit stays in Saved fits.");
  });

  it("opens a saved fit from the list and sends Import first to the import screen", async () => {
    const user = userEvent.setup();
    const s = setup("/profiles/7/harnfit/lib%3Alite", {
      saved: fitView({
        missingMcp: [{ server: "postgres", inLibrary: false }],
      }),
      summaries: [
        {
          candidate: {
            key: "lib:other",
            label: "other",
            kind: "skill",
            source: "Library",
          },
          verdict: "fits",
          createdAt: 1_730_000_000,
          stale: false,
        },
      ],
    });
    await screen.findByRole("region", { name: "Verdict for tdd-lite" });
    await user.click(screen.getByRole("button", { name: /other/ }));
    await waitFor(() =>
      expect(ran(s, "harnfit_get").map((c) => c.args)).toContainEqual({
        profileId: 7,
        candidateKey: "lib:other",
      }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Import postgres first" }),
    );
    expect((await screen.findByTestId("where")).textContent).toContain(
      "/import",
    );
  });
});

describe("Harnfit screen: states", () => {
  it("needs a map", async () => {
    const user = userEvent.setup();
    const s = setup(
      "/profiles/7/harnfit/lib%3Alite",
      { drawn: false },
      { name: "tdd-lite" },
    );
    const card = await screen.findByRole("region", {
      name: "Harnfit needs a map",
    });
    expect(
      within(card).getByText(
        /To fit tdd-lite into Heavy, Heavy needs a map first\./,
      ),
    ).toBeVisible();
    expect(ran(s, "harnfit_run")).toHaveLength(0);
    await user.click(
      within(card).getByRole("button", { name: "Draw Heavy’s map first" }),
    );
    expect((await screen.findByTestId("where")).textContent).toContain(
      "/profiles/7/harnmap",
    );
  });

  it("maps core's refusals to their cards", async () => {
    setup("/profiles/7/harnfit/lib%3Alite", {
      fail: {
        type: "other",
        message: "the profile has no map yet: draw it first",
      },
    });
    expect(
      await screen.findByRole("region", { name: "Harnfit needs a map" }),
    ).toBeVisible();
  });

  it("shows plugins as not supported yet", async () => {
    setup("/profiles/7/harnfit/lib%3Aplug", {
      fail: {
        type: "other",
        message: "Harnfit does not take a plugin whole yet; fit its skills",
      },
    });
    expect(
      await screen.findByText(/Harnfit for plugins comes later/),
    ).toBeVisible();
  });

  it("shows a failed run and retries", async () => {
    const user = userEvent.setup();
    const s = setup("/profiles/7/harnfit/lib%3Alite", {
      fail: { type: "badAnswer", reasons: ["no verdict"] },
    });
    const card = await screen.findByRole("region", { name: "Harnfit failed" });
    expect(within(card).getByText("no verdict")).toBeVisible();
    await user.click(within(card).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(ran(s, "harnfit_run")).toHaveLength(2));
  });

  it("shows a missing claude CLI and a failed claude", async () => {
    setup("/profiles/7/harnfit/lib%3Alite", {
      fail: { type: "claudeMissing" },
    });
    expect(await screen.findByText("claude CLI not found")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Claude Code setup guide" }),
    ).toBeVisible();
  });

  it("shows other failures with their message", async () => {
    setup("/profiles/7/harnfit/lib%3Alite", {
      fail: { type: "claudeFailed", kind: "limit", message: "Usage limit." },
    });
    expect(await screen.findByText(/Usage limit\./)).toBeVisible();
  });
});
