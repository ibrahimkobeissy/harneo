import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { afterEach, describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Page } from "../../bindings/Page";
import type { Query } from "../../bindings/Query";
import type { Usage } from "../../bindings/Usage";
import type { UsageSummary } from "../../bindings/UsageSummary";
import { UsageIndexProvider } from "../../app/usageIndex";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";
import { relativeDay } from "./usage";

const summary: UsageSummary = {
  thisWeek: 12,
  changePct: 20,
  weekly: [1, 2, 3, 4, 5, 6, 10, 12],
};

const items: LibraryItem[] = [
  item("busy", { usage: { sessions: 7, lastUsed: "2026-10-20" } }),
  item("idle", {
    status: "unused",
    usage: { sessions: 0, lastUsed: "2026-08-01" },
  }),
  item("fresh", {
    status: "notUsedYet",
    usage: { sessions: 0, lastUsed: null },
  }),
  item("hooky", {}, "settings"),
];

const facets: Page["facets"] = {
  ...page([]).facets,
  statuses: [
    { status: "findings", count: 1 },
    { status: "outdated", count: 2 },
    { status: "unused", count: 3 },
    { status: "notUsedYet", count: 1 },
    { status: "current", count: 10 },
  ],
};

function Probe() {
  return <span data-testid="search">{useLocation().search}</span>;
}

function setup(
  opts: {
    indexedAt?: string | null;
    summary?: UsageSummary;
    usage?: Usage;
    path?: string;
  } = {},
) {
  const queries: Query[] = [];
  const calls: string[] = [];
  mockIPC((cmd, args) => {
    calls.push(cmd);
    switch (cmd) {
      case "library_list":
        queries.push((args as { query: Query }).query);
        return page(items, { facets, total: 4 });
      case "settings_get":
        return opts.indexedAt === undefined
          ? "2026-10-23T10:00:00Z"
          : opts.indexedAt;
      case "usage_index":
        return new Promise(() => {});
      case "usage_summary":
        return opts.summary ?? summary;
      case "usage_of":
        return (
          opts.usage ?? {
            sessions8w: 7,
            weekly: [0, 0, 1, 0, 2, 0, 1, 3],
            lastUsed: "2026-10-20",
            projects: ["/home/me/app", "/home/me/site"],
          }
        );
      case "library_detail":
        return detail((args as { id: string }).id);
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[opts.path ?? "/library"]}>
      <Probe />
      <UsageIndexProvider>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
          <Route path="/cleanup" element={<h1>Cleanup screen</h1>} />
        </Routes>
      </UsageIndexProvider>
    </MemoryRouter>,
  );
  return { queries, calls };
}

function detail(id: string): ArtifactDetail {
  const name = id.replace("id-", "");
  return {
    id,
    artifact: {
      kind: name === "hooky" ? "settings" : "skill",
      name,
      description: null,
      path: `skills/${name}`,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    audit: null,
    auditedAt: null,
    links: { needs: [], neededBy: [] },
    manualOnly: false,
    provenance: {
      source: {
        id: "s1",
        kind: "local",
        location: "/x",
        branch: null,
        subfolder: null,
        commitSha: null,
      },
      sourcePath: "x",
      digest: null,
      importedAt: 1_700_000_000n,
      replacedSecretKeys: [],
    },
    files: [],
  };
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("relativeDay", () => {
  const now = new Date(2026, 9, 23);
  it("speaks in days, weeks, months and years", () => {
    expect(relativeDay("2026-10-23", now)).toBe("today");
    expect(relativeDay("2026-10-22", now)).toBe("yesterday");
    expect(relativeDay("2026-10-18", now)).toBe("5 days ago");
    expect(relativeDay("2026-10-02", now)).toBe("3 weeks ago");
    expect(relativeDay("2026-06-23", now)).toBe("4 months ago");
    expect(relativeDay("2024-10-23", now)).toBe("2 years ago");
    expect(relativeDay("soon", now)).toBe("soon");
  });
});

describe("cards", () => {
  it("shows sessions, unused, not used yet and not tracked in the footers", async () => {
    setup();
    const busy = await screen.findByRole("button", { name: /busy/ });
    expect(busy).toHaveTextContent("7 sessions");
    const idle = screen.getByRole("button", { name: /idle/ });
    expect(idle).toHaveTextContent(/unused · last \d+ (months|weeks) ago/);
    expect(idle).toHaveAttribute("data-unused");
    expect(screen.getByRole("button", { name: /fresh/ })).toHaveTextContent(
      "not used yet",
    );
    expect(screen.getByRole("button", { name: /hooky/ })).toHaveTextContent(
      "Not tracked",
    );
  });

  it("does not call anything unused before the first index has completed", async () => {
    setup({ indexedAt: null });
    const idle = await screen.findByRole("button", { name: /idle/ });
    expect(idle).not.toHaveAttribute("data-unused");
    expect(idle).not.toHaveTextContent("unused");
    expect(
      await screen.findByText(/Reading Claude's session logs/),
    ).toBeVisible();
    const health = screen.getByRole("region", { name: "Library health" });
    expect(within(health).getAllByText("reading logs")).toHaveLength(2);
    expect(
      within(health).getByRole("button", {
        name: "Filter by unused: reading logs",
      }),
    ).toBeVisible();
  });
});

describe("list", () => {
  it("has Sessions and Status columns", async () => {
    setup({ path: "/library?view=list" });
    const table = await screen.findByRole("table");
    const rows = within(table).getAllByRole("row");
    expect(rows[1]).toHaveTextContent("7");
    expect(rows[2]).toHaveTextContent("Unused");
    expect(rows[4]).toHaveTextContent("—");
    expect(within(rows[1] as HTMLElement).getByText("7").className).toMatch(
      /sessionsCell/,
    );
    expect(
      within(table).getByRole("columnheader", { name: /Sessions/ }).className,
    ).toMatch(/sessionsHead/);
    expect(
      within(table).getByRole("button", { name: "Sessions" }),
    ).toHaveAttribute("title", "Sessions in the last 8 weeks");
  });
});

describe("sort and status filter", () => {
  it("sorts by Most used, highest first", async () => {
    const user = userEvent.setup();
    const { queries } = setup();
    await screen.findByRole("button", { name: /busy/ });
    await user.click(screen.getByRole("button", { name: "Most used" }));
    await waitFor(() => expect(queries.at(-1)?.sort).toBe("sessions"));
    expect(queries.at(-1)?.descending).toBe(true);
  });

  it("filters by status from the filter column, with counts", async () => {
    const user = userEvent.setup();
    const { queries } = setup();
    await screen.findByRole("button", { name: /busy/ });
    const aside = screen.getByRole("complementary", { name: "Filters" });
    const unused = within(aside).getByRole("button", { name: /Unused/ });
    expect(unused).toHaveTextContent("3");
    await user.click(unused);
    await waitFor(() =>
      expect(queries.at(-1)?.filter.statuses).toEqual(["unused"]),
    );
    expect(screen.getByTestId("search")).toHaveTextContent("status=unused");
  });
});

describe("library insights", () => {
  it("draws the health bar, filters from its legend and links to Cleanup", async () => {
    const user = userEvent.setup();
    const { queries } = setup();
    const health = await screen.findByRole("region", {
      name: "Library health",
    });
    expect(within(health).getByText("findings")).toBeVisible();
    expect(within(health).getByRole("img")).toHaveAccessibleName(
      "11 current, 3 unused, 2 outdated, 1 with findings",
    );
    await user.click(
      within(health).getByRole("button", { name: /Filter by outdated/ }),
    );
    await waitFor(() =>
      expect(queries.at(-1)?.filter.statuses).toEqual(["outdated"]),
    );
    await user.click(
      within(health).getByRole("link", { name: "Review 6 issues" }),
    );
    expect(await screen.findByText("Cleanup screen")).toBeVisible();
  });

  it("charts 8 weeks of sessions with this week's change", async () => {
    setup();
    const chart = await screen.findByRole("img", {
      name: "Sessions per week for 8 weeks, latest 12",
    });
    expect(chart.querySelectorAll("i")).toHaveLength(8);
    expect(screen.getByText("12 this week · +20%")).toBeVisible();
  });

  it("prints the max and the latest value on the chart", async () => {
    setup();
    await screen.findByRole("img", { name: /Sessions per week/ });
    expect(screen.getByText("max 12")).toBeVisible();
    const latest = document.querySelector("i[data-now] b");
    expect(latest).toHaveTextContent("12");
  });

  it("explains an empty chart", async () => {
    setup({
      summary: {
        thisWeek: 0,
        changePct: null,
        weekly: [0, 0, 0, 0, 0, 0, 0, 0],
      },
    });
    expect(await screen.findByText(/No sessions recorded yet/)).toBeVisible();
  });
});

describe("detail panel", () => {
  it("shows Usage with bars and projects for a tracked kind", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: /busy/ }));
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByText(/7 sessions in 8 weeks · last/),
    ).toBeVisible();
    expect(
      within(dialog)
        .getByRole("img", { name: /Sessions per week/ })
        .querySelectorAll("i"),
    ).toHaveLength(8);
    expect(within(dialog).getByText("Seen in 2 projects")).toBeVisible();
    expect(within(dialog).getAllByText(/^Used in/)).toHaveLength(1);
    // The max and the latest value are printed, not only in tooltips.
    expect(within(dialog).getByText("max 3")).toBeVisible();
    expect(within(dialog).getByText("/home/me/app")).toBeVisible();
  });

  it("says why an untracked kind has no usage", async () => {
    const user = userEvent.setup();
    const { calls } = setup();
    await user.click(await screen.findByRole("button", { name: /hooky/ }));
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByText(
        "Not tracked: Claude's logs don't record when settings are used.",
      ),
    ).toBeVisible();
    expect(calls).not.toContain("usage_of");
  });
});
