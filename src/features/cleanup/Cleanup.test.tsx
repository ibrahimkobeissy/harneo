import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import type { Cleanup } from "../../bindings/Cleanup";
import type { CleanupItem } from "../../bindings/CleanupItem";
import { UsageIndexContext, type UsageIndexState } from "../../app/usageIndex";
import { ArtifactPanelProvider } from "../../lib/artifactPanel";
import { ArtifactPanelHost } from "../../app/ArtifactPanelHost";
import { minimalDetail } from "../../test/artifactDetail";
import { CleanupScreen } from ".";

/** The local day 70 days ago, which always reads "2 months ago", whatever today is. */
function seventyDaysAgo(): string {
  const d = new Date();
  d.setDate(d.getDate() - 70);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function item(over: Partial<CleanupItem>): CleanupItem {
  return {
    id: "a1",
    kind: "skill",
    name: "webperf",
    trustScore: null,
    findingCount: 0,
    fromVersion: null,
    toVersion: null,
    localEdits: false,
    lastUsed: null,
    profiles: [],
    ...over,
  };
}

const empty: Cleanup = {
  findings: [],
  outdated: [],
  unused: [],
  repeated: [],
  duplicates: [],
};

interface Setup {
  data: Cleanup;
  failList?: boolean;
  planFails?: boolean;
  applyFails?: boolean;
  failedReport?: boolean;
  updateOutcome?: string;
  updateThrows?: boolean;
  updateAllThrows?: boolean;
  weeks?: unknown;
  lastRead?: string;
}

function setup(s: Setup, usage: Partial<UsageIndexState> = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "cleanup_list":
        if (s.failList) throw new Error("db locked");
        return s.data;
      case "settings_get":
        if ((args as { key?: string })["key"] === "usage.lastIndexedAt")
          return s.lastRead ?? null;
        return s.weeks ?? null;
      case "prune_plan":
        if (s.planFails) throw new Error("no plan");
        return {
          artifact: "u1",
          artifactName: "webperf",
          profiles: [
            { id: 1, name: "Heavy", fromVersion: 3, toVersion: 4 },
            { id: 2, name: "Light", fromVersion: 1, toVersion: 2 },
          ],
          projects: [{ id: 5, name: "shop", profileId: 1, plan: {} }],
          notSwitched: [
            { id: 6, name: "blog", profileId: 2, profileVersion: 1 },
          ],
        };
      case "prune_apply":
        if (s.applyFails) throw new Error("disk full");
        return {
          profiles: [{ id: 1, name: "Heavy", fromVersion: 3, toVersion: 4 }],
          switched: [{ projectId: 5, name: "shop", report: {} }],
          failed: s.failedReport ? { subject: "Light", message: "boom" } : null,
          notSwitched: [],
        };
      case "update_apply":
        if (s.updateThrows) throw new Error("offline");
        return {
          id: "o1",
          outcome: s.updateOutcome ?? "updated",
          message: "bad",
        };
      case "update_all":
        if (s.updateAllThrows) throw new Error("offline");
        return [
          { id: "o1", outcome: "updated" },
          { id: "o2", outcome: "skippedLocalEdits" },
        ];
      case "tags_list":
      case "artifact_tags":
        return [];
      case "library_detail":
        return minimalDetail((args as { id: string }).id);
      case "library_remove":
        if (s.applyFails) throw new Error("locked");
        return undefined;
      default:
        return undefined;
    }
  });
  render(
    <UsageIndexContext.Provider
      value={{
        indexed: true,
        progress: null,
        error: null,
        runs: 0,
        refresh: () => {},
        ...usage,
      }}
    >
      <MemoryRouter>
        <ArtifactPanelProvider>
          <CleanupScreen />
          <ArtifactPanelHost />
        </ArtifactPanelProvider>
      </MemoryRouter>
    </UsageIndexContext.Provider>,
  );
  return calls;
}

const used = [
  { id: 1n, name: "Heavy", version: 3 },
  { id: 2n, name: "Light", version: 1 },
] as unknown as CleanupItem["profiles"];

describe("Cleanup groups", () => {
  it("shows the empty state", async () => {
    setup({ data: empty });
    expect(await screen.findByText("Nothing to clean up")).toBeVisible();
    expect(screen.getByRole("heading", { name: "Cleanup" })).toBeVisible();
  });

  it("shows an error when the suggestions do not load", async () => {
    setup({ data: empty, failList: true });
    expect(await screen.findByRole("alert")).toHaveTextContent("db locked");
  });

  it("lists every group with counts", async () => {
    const lastUsed = seventyDaysAgo();
    setup({
      weeks: 6,
      data: {
        findings: [
          item({ id: "f1", name: "shady", trustScore: 30, findingCount: 2 }),
        ],
        outdated: [
          item({ id: "o1", name: "fresh", fromVersion: "1", toVersion: "2" }),
          item({ id: "o2", name: "edited", localEdits: true }),
        ],
        unused: [
          item({
            id: "u1",
            name: "webperf",
            profiles: used,
            lastUsed,
          }),
          item({ id: "u2", name: "orphan" }),
        ],
        repeated: [item({ id: "r1", name: "common", profiles: used })],
        duplicates: [{ name: "twin", kind: "skill", ids: ["x", "y"] }],
      },
    });
    expect(await screen.findByText("shady")).toBeVisible();
    expect(screen.getByText("2 findings")).toBeVisible();
    expect(screen.getByText("1 → 2")).toBeVisible();
    expect(screen.getByText("local edits")).toBeVisible();
    expect(
      await screen.findByRole("heading", {
        name: "Unused for 6+ weeks",
      }),
    ).toBeVisible();
    expect(screen.getAllByText("in Heavy, Light")).toHaveLength(2);
    expect(screen.getByText("in no profile")).toBeVisible();
    expect(screen.getByText("not used yet")).toBeVisible();
    expect(screen.getByText("last used 2 months ago")).toHaveAttribute(
      "title",
      lastUsed,
    );
    expect(screen.getByText(/Not a problem in itself/)).toBeVisible();
    expect(screen.getByText("2 copies")).toBeVisible();
    expect(
      screen.getByRole("link", { name: "Show in library" }),
    ).toHaveAttribute("href", "/library?q=twin");
    expect(
      screen.getAllByRole("button", { name: "shady" })[0],
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open profiles" })).toBeVisible();
  });

  it("opens an artifact's details in place and gives focus back on Esc", async () => {
    setup({
      data: { ...empty, findings: [item({ id: "f1", name: "shady" })] },
    });
    const row = (await screen.findAllByRole("button", { name: "shady" }))[0]!;
    await userEvent.click(row);
    expect(
      await screen.findByRole("button", { name: "Close details" }),
    ).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Close details" }),
      ).toBeNull(),
    );
    expect(row).toHaveFocus();
  });

  it("shows the first 12 unused and links to the rest", async () => {
    const unused = Array.from({ length: 14 }, (_, i) =>
      item({ id: `u${i}`, name: `n${i}` }),
    );
    setup({ data: { ...empty, unused } });
    expect(await screen.findByText("n11")).toBeVisible();
    expect(screen.queryByText("n12")).toBeNull();
    expect(
      screen.getByRole("link", { name: "See all 14 in the library" }),
    ).toHaveAttribute("href", "/library?status=unused");
  });
});

describe("Cleanup actions", () => {
  it("removes a risky artifact from the library after a confirm", async () => {
    const calls = setup({
      data: {
        ...empty,
        findings: [item({ id: "f1", name: "shady", trustScore: 70 })],
      },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from library" }),
    );
    const dlg = screen.getByRole("dialog");
    expect(within(dlg).getByText(/It is in no profile/)).toBeVisible();
    await userEvent.click(
      within(dlg).getByRole("button", { name: "Remove from library" }),
    );
    expect(
      await screen.findByText("Removed shady from the library"),
    ).toBeVisible();
    expect(calls.some((c) => c.cmd === "library_remove")).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Removed shady from the library")).toBeNull();
  });

  it("prunes a risky artifact that profiles install, from its row", async () => {
    setup({
      data: {
        ...empty,
        findings: [
          item({ id: "f1", name: "shady", findingCount: 3, profiles: used }),
        ],
      },
    });
    expect(
      screen.queryByRole("button", { name: "Remove from library" }),
    ).toBeNull();
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from profiles" }),
    );
    expect(
      await screen.findByRole("dialog", {
        name: "Remove shady from 2 profiles?",
      }),
    ).toBeVisible();
  });

  it("opens the security screen from a findings row", async () => {
    setup({
      data: { ...empty, findings: [item({ id: "f1", name: "shady" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "View findings" }),
    );
  });

  it("reviews an edited outdated artifact in the library", async () => {
    setup({
      data: {
        ...empty,
        outdated: [item({ id: "o2", name: "edited", localEdits: true })],
      },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Review" }),
    );
  });

  it("shows a library removal failure and keeps the dialog", async () => {
    setup({
      applyFails: true,
      data: { ...empty, unused: [item({ id: "u2", name: "orphan" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from library" }),
    );
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Remove from library",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("locked");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("plans, confirms and applies a prune", async () => {
    const calls = setup({
      data: {
        ...empty,
        unused: [item({ id: "u1", name: "webperf", profiles: used })],
      },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from profiles" }),
    );
    const dlg = await screen.findByRole("dialog", {
      name: "Remove webperf from 2 profiles?",
    });
    expect(await within(dlg).findByText("v3 → v4")).toBeVisible();
    expect(within(dlg).getByText("shop")).toBeVisible();
    expect(within(dlg).getByText(/blog stays on/)).toBeVisible();
    await userEvent.click(
      within(dlg).getByRole("button", { name: "Remove and push to 1 project" }),
    );
    expect(
      await screen.findByText(
        "Removed webperf from 1 profile and switched 1 project",
      ),
    ).toBeVisible();
    expect(calls.some((c) => c.cmd === "prune_apply")).toBe(true);
  });

  it("shows a prune that stopped halfway", async () => {
    setup({
      failedReport: true,
      data: { ...empty, unused: [item({ id: "u1", profiles: used })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from profiles" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /Remove and push/ }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Stopped at Light: boom",
    );
  });

  it("shows plan and apply errors", async () => {
    setup({
      planFails: true,
      data: { ...empty, unused: [item({ id: "u1", profiles: used })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from profiles" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("no plan");
  });

  it("shows an apply error and lets the user retry", async () => {
    setup({
      applyFails: true,
      data: { ...empty, unused: [item({ id: "u1", profiles: used })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from profiles" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /Remove and push/ }),
    );
    expect(await screen.findByText("disk full")).toBeVisible();
  });

  it("updates one artifact", async () => {
    const calls = setup({
      data: { ...empty, outdated: [item({ id: "o1", name: "fresh" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Update" }),
    );
    expect(await screen.findByText("Updated fresh")).toBeVisible();
    expect(calls.find((c) => c.cmd === "update_apply")?.args).toEqual({
      id: "o1",
      choice: "takeUpstream",
    });
  });

  it("reports a failed single update and a thrown one", async () => {
    setup({
      updateOutcome: "failed",
      data: { ...empty, outdated: [item({ id: "o1", name: "fresh" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Update" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not update fresh. bad",
    );
  });

  it("reports a thrown single update", async () => {
    setup({
      updateThrows: true,
      data: { ...empty, outdated: [item({ id: "o1", name: "fresh" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Update" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("offline");
  });

  it("updates all and explains the skipped ones", async () => {
    setup({
      data: {
        ...empty,
        outdated: [
          item({ id: "o1", name: "fresh" }),
          item({ id: "o2", name: "edited", localEdits: true }),
        ],
      },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Update all" }),
    );
    const dlg = screen.getByRole("dialog", {
      name: "Update 1 of 2 artifacts?",
    });
    expect(
      within(dlg).getByText(/has local edits and is skipped/),
    ).toBeVisible();
    expect(
      within(dlg).getByRole("button", { name: "edited" }),
    ).toBeInTheDocument();
    await userEvent.click(
      within(dlg).getByRole("button", { name: "Update 1" }),
    );
    expect(await screen.findByText("Updated 1 artifact")).toBeVisible();
  });

  it("reports an update-all failure", async () => {
    setup({
      updateAllThrows: true,
      data: { ...empty, outdated: [item({ id: "o1" })] },
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Update all" }),
    );
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Update 1",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not update",
    );
  });
});

describe("Cleanup and the usage index", () => {
  const unused = [item({ id: "u1", name: "old", profiles: used })];

  it("lists no unused artifacts until the first index run", async () => {
    setup(
      { data: { ...empty, unused } },
      {
        indexed: false,
        progress: { filesDone: 12, filesTotal: 40, newUses: 0 },
      },
    );
    expect(
      await screen.findByText(/Reading Claude's session logs/),
    ).toBeVisible();
    expect(screen.getByText(/12 of 40 files/)).toBeVisible();
    expect(screen.queryByText("old")).toBeNull();
    expect(screen.queryByText("Nothing to clean up")).toBeNull();
  });

  it("waits quietly before the first run reports progress", async () => {
    setup({ data: { ...empty, unused } }, { indexed: false });
    expect(
      await screen.findByText(/Reading Claude's session logs/),
    ).toBeVisible();
  });

  it("says the logs could not be read and retries", async () => {
    const refresh = vi.fn();
    setup(
      { data: { ...empty, unused } },
      { indexed: false, error: "no folder", refresh },
    );
    expect(await screen.findByText(/no folder/)).toBeVisible();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(refresh).toHaveBeenCalled();
    expect(screen.queryByText("old")).toBeNull();
  });

  it("says when usage was last read in the empty state", async () => {
    setup({ data: empty, lastRead: new Date().toISOString() });
    expect(await screen.findByText("Nothing to clean up")).toBeVisible();
    expect(await screen.findByText(/Usage last read today/)).toBeVisible();
  });

  it("retries a failed load", async () => {
    const calls = setup({ data: empty, failList: true });
    await userEvent.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() =>
      expect(calls.filter((c) => c.cmd === "cleanup_list").length).toBe(2),
    );
  });
});

describe("Cleanup dialogs while applying", () => {
  it("ignores Esc and Cancel while the apply runs", async () => {
    let finishApply: () => void = () => {};
    mockIPC((cmd) => {
      if (cmd === "cleanup_list")
        return { ...empty, unused: [item({ id: "u2", name: "orphan" })] };
      if (cmd === "settings_get") return null;
      if (cmd === "library_remove")
        return new Promise<void>((resolve) => {
          finishApply = resolve;
        });
      return undefined;
    });
    render(
      <MemoryRouter>
        <CleanupScreen />
      </MemoryRouter>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove from library" }),
    );
    const dlg = screen.getByRole("dialog");
    await userEvent.click(
      within(dlg).getByRole("button", { name: "Remove from library" }),
    );
    expect(
      await within(dlg).findByRole("button", { name: "Cancel" }),
    ).toBeDisabled();
    expect(dlg.querySelector("form")).toHaveAttribute("aria-busy", "true");
    await userEvent.keyboard("{Escape}");
    await userEvent.click(
      document.querySelector("[aria-hidden='true'][class*='scrim']") ??
        document.body,
    );
    expect(screen.getByRole("dialog")).toBeVisible();
    finishApply();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
