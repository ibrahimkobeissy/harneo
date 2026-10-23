import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  MemoryRouter,
  Route,
  Routes,
  useLocation,
  useNavigate,
} from "react-router";
import { describe, expect, it, vi } from "vitest";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Page } from "../../bindings/Page";
import type { Query } from "../../bindings/Query";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const facets: Page["facets"] = {
  tiers: [],
  withFindings: 0,
  outdated: 0,
  statuses: [],
  kinds: [
    { kind: "skill", count: 5 },
    { kind: "agent", count: 2 },
  ],
  tags: [
    { name: "review", color: "violet", count: 3 },
    { name: "docs", color: "cyan", count: 1 },
  ],
  sources: [
    {
      source: { id: "s1", kind: "local", label: "agent-skills", branch: null },
      count: 4,
    },
    {
      source: { id: "s2", kind: "git", label: "acme/kit", branch: "main" },
      count: 3,
    },
  ],
};

const sample: LibraryItem[] = [
  item("alpha", {
    description: "Reviews pull requests",
    tags: [
      { name: "review", color: "violet" },
      { name: "docs", color: "cyan" },
      { name: "extra", color: "slate" },
    ],
    errorCount: 2,
    warningCount: 1,
    fileCount: 4,
    referenceCount: 3,
    resolvedReferences: 2,
    source: { id: "s2", kind: "git", label: "acme/kit", branch: "main" },
  }),
  item("beta", { description: null }, "agent"),
];

function Probe() {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <>
      <span data-testid="search">{location.search}</span>
      <button type="button" onClick={() => void navigate(-1)}>
        history back
      </button>
    </>
  );
}

function setup(
  respond: (query: Query) => Page | Promise<Page>,
  path = "/library",
  onOpen?: (item: LibraryItem) => void,
) {
  const queries: Query[] = [];
  mockIPC((cmd, args) => {
    if (cmd !== "library_list") return undefined;
    const query = (args as { query: Query }).query;
    queries.push(query);
    return respond(query);
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
      <Routes>
        <Route path="/library" element={<LibraryScreen onOpen={onOpen} />} />
        <Route path="/import" element={<h1>Import screen</h1>} />
      </Routes>
    </MemoryRouter>,
  );
  const search = () => screen.getByTestId("search").textContent;
  const last = (): Query => must(queries.at(-1));
  return { queries, search, last };
}

function must<T>(value: T | undefined | null): T {
  if (value === undefined || value === null) throw new Error("missing value");
  return value;
}

const full = () => page(sample, { total: 7, facets });

describe("keyboard", () => {
  it("focuses the search with / but not while typing in a field", async () => {
    const user = userEvent.setup();
    setup(full);
    await screen.findByRole("button", { name: /alpha/ });
    const search = screen.getByRole("searchbox", { name: "Search artifacts" });
    await user.keyboard("/");
    expect(search).toHaveFocus();
    expect(search).toHaveValue("");
    await user.keyboard("a/b");
    expect(search).toHaveValue("a/b");
  });

  it("moves focus between cards with the arrow keys, Home and End", async () => {
    const user = userEvent.setup();
    setup(full);
    const alpha = await screen.findByRole("button", { name: /alpha/ });
    const beta = screen.getByRole("button", { name: /beta/ });
    const grid = must(alpha.parentElement);
    grid.style.gridTemplateColumns = "100px 100px";
    alpha.focus();
    await user.keyboard("{ArrowRight}");
    expect(beta).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(alpha).toHaveFocus();
    await user.keyboard("{End}");
    expect(beta).toHaveFocus();
    await user.keyboard("{Home}");
    expect(alpha).toHaveFocus();
    // Down moves by a row: two columns, two cards, so it stays on the last card.
    await user.keyboard("{ArrowDown}");
    expect(beta).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(alpha).toHaveFocus();
  });

  it("goes down a whole row, not a card, when the grid has columns", async () => {
    const user = userEvent.setup();
    setup(() => page([item("a1"), item("a2"), item("a3"), item("a4")]));
    const first = await screen.findByRole("button", { name: /a1/ });
    const grid = must(first.parentElement);
    grid.style.gridTemplateColumns = "100px 100px";
    first.focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: /a3/ })).toHaveFocus();
  });
});

describe("cards view", () => {
  it("does not repeat the name as the type label", async () => {
    setup(() =>
      page([item("Settings", { description: null }, "settings"), item("beta")]),
    );
    const card = await screen.findByRole("button", { name: /Settings/ });
    expect(card.textContent?.match(/Settings/g)).toHaveLength(1);
  });

  it("renders each item with its kind, description, source, tags and counts", async () => {
    setup(full);
    expect(
      await screen.findByRole("button", { name: /alpha/ }),
    ).toBeInTheDocument();
    const card = screen.getByRole("button", { name: /alpha/ });
    expect(card).toHaveTextContent("Skill");
    expect(card).toHaveTextContent("Reviews pull requests");
    expect(card).toHaveTextContent("acme/kit @ main");
    expect(card).toHaveTextContent("review");
    expect(card).toHaveTextContent("+1");
    expect(card).toHaveTextContent("2 scan errors");
    expect(card).toHaveTextContent("1 scan warning");
    expect(card).toHaveTextContent("4 files · 2/3 refs");
    const second = screen.getByRole("button", { name: /beta/ });
    expect(second).toHaveTextContent("Subagent");
    expect(second).not.toHaveTextContent("error");
    expect(screen.getByText("7 artifacts")).toBeInTheDocument();
  });

  it("asks for 24 cards by name on the first page", async () => {
    const { last } = setup(full);
    await screen.findByText("alpha");
    expect(last()).toMatchObject({
      sort: "name",
      descending: false,
      offset: 0,
      limit: 24,
    });
  });

  it("sorts from the sort buttons and flips the direction", async () => {
    const user = userEvent.setup();
    const { last, search } = setup(full);
    await screen.findByText("alpha");
    const group = screen.getByRole("group", { name: "Sort by" });
    expect(within(group).getByRole("button", { name: /Name/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(group).getByRole("button", { name: "Imported" }));
    await waitFor(() => expect(last().sort).toBe("importedAt"));
    expect(last().descending).toBe(true);
    await user.click(
      within(group).getByRole("button", { name: /Imported, descending/ }),
    );
    await waitFor(() => expect(last().descending).toBe(false));
    expect(search()).toContain("sort=importedAt");
  });

  it("calls onOpen when a card is activated with Enter", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    setup(full, "/library", onOpen);
    const card = await screen.findByRole("button", { name: /alpha/ });
    card.focus();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenCalledWith(sample[0]);
  });
});

describe("list view", () => {
  it("renders rows with sortable headers and 25 rows per page", async () => {
    const { last } = setup(full, "/library?view=list");
    const table = await screen.findByRole("table");
    expect(last().limit).toBe(25);
    const headers = within(table).getAllByRole("columnheader");
    expect(headers.map((h) => h.textContent)).toEqual([
      "Name",
      "Type",
      "Source",
      "Tags",
      "Trust",
      "Imported",
      "Bundle",
    ]);
    expect(must(headers[0])).toHaveAttribute("aria-sort", "ascending");
    expect(must(headers[1])).toHaveAttribute("aria-sort", "none");
    const rows = within(table).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(must(rows[1])).toHaveTextContent("alpha");
    expect(must(rows[1])).toHaveTextContent("Skill");
    expect(must(rows[1])).toHaveTextContent("acme/kit @ main");
    // The name is a real button; the row itself is not a tab stop.
    expect(must(rows[1])).not.toHaveAttribute("tabindex");
    expect(
      within(must(rows[1])).getByRole("button", { name: "alpha" }),
    ).toBeInTheDocument();
    expect(rows.every((r) => r.querySelector("[tabindex]") === null)).toBe(
      true,
    );
    // Dates read "21 Oct 2025", never 10/21/25.
    expect(must(rows[1])).toHaveTextContent(
      /\d{1,2}\s\w{3}\s\d{4}|\w{3}\s\d{1,2},\s\d{4}/,
    );
  });

  it.each([
    ["Type", "kind", false],
    ["Source", "source", false],
    ["Tags", "tagCount", false],
    ["Imported", "importedAt", true],
  ])(
    "sorts by %s, then toggles the direction",
    async (label, key, firstDescending) => {
      const user = userEvent.setup();
      const { last } = setup(full, "/library?view=list");
      await screen.findByRole("table");
      const header = () =>
        screen.getByRole("columnheader", { name: new RegExp(label) });
      await user.click(within(header()).getByRole("button"));
      await waitFor(() => expect(last().sort).toBe(key));
      expect(last().descending).toBe(firstDescending);
      expect(header()).toHaveAttribute(
        "aria-sort",
        firstDescending ? "descending" : "ascending",
      );
      await user.click(within(header()).getByRole("button"));
      await waitFor(() => expect(last().descending).toBe(!firstDescending));
      expect(header()).toHaveAttribute(
        "aria-sort",
        firstDescending ? "ascending" : "descending",
      );
    },
  );

  it("sorts by name descending after a second click", async () => {
    const user = userEvent.setup();
    const { last } = setup(full, "/library?view=list");
    await screen.findByRole("table");
    await user.click(screen.getByRole("button", { name: "Name" }));
    await waitFor(() => expect(last().descending).toBe(true));
    expect(last().sort).toBe("name");
  });

  it("opens a row with Enter", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    setup(full, "/library?view=list", onOpen);
    const name = await screen.findByRole("button", { name: "alpha" });
    name.focus();
    await user.keyboard("{Enter}");
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith(sample[0]);
  });

  it("opens once when the row itself is clicked", async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    setup(full, "/library?view=list", onOpen);
    const row = must((await screen.findAllByRole("row"))[1]);
    await user.click(within(row).getByText("acme/kit @ main"));
    expect(onOpen).toHaveBeenCalledTimes(1);
    await user.click(within(row).getByRole("button", { name: "alpha" }));
    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it("switches between cards and list through the toggle", async () => {
    const user = userEvent.setup();
    const { search } = setup(full);
    await screen.findByText("alpha");
    await user.click(screen.getByRole("button", { name: "List" }));
    expect(await screen.findByRole("table")).toBeInTheDocument();
    expect(search()).toBe("?view=list");
    await user.click(screen.getByRole("button", { name: "Cards" }));
    await waitFor(() => expect(screen.queryByRole("table")).toBeNull());
    expect(search()).toBe("");
  });
});

describe("filters and search", () => {
  it("lists facet counts in the filter column", async () => {
    setup(full);
    const column = await screen.findByRole("complementary", {
      name: "Filters",
    });
    expect(
      within(column).getByRole("button", { name: /Skills\s*5/ }),
    ).toBeInTheDocument();
    expect(
      within(column).getByRole("button", { name: /review\s*3/ }),
    ).toBeInTheDocument();
    expect(
      within(column).getByRole("button", { name: /acme\/kit @ main\s*3/ }),
    ).toBeInTheDocument();
  });

  it("narrows by type, tag and source and keeps them in the URL", async () => {
    const user = userEvent.setup();
    const { last, search } = setup(full);
    const column = await screen.findByRole("complementary");
    await user.click(within(column).getByRole("button", { name: /Skills/ }));
    await waitFor(() => expect(last().filter.types).toEqual(["skill"]));
    await user.click(within(column).getByRole("button", { name: /review/ }));
    await waitFor(() => expect(last().filter.tags).toEqual(["review"]));
    await user.click(within(column).getByRole("button", { name: /acme/ }));
    await waitFor(() => expect(last().filter.sources).toEqual(["s2"]));
    expect(search()).toBe("?type=skill&tag=review&source=s2");
    expect(
      within(column).getByRole("button", { name: /Skills/ }),
    ).toHaveAttribute("aria-pressed", "true");
    await user.click(within(column).getByRole("button", { name: /Skills/ }));
    await waitFor(() => expect(last().filter.types).toEqual([]));
  });

  it("shows one removable chip per filter with an N of M count", async () => {
    const user = userEvent.setup();
    const { last } = setup(
      (q) =>
        page(sample, {
          total: q.filter.tags.length ? 2 : 7,
          unfilteredTotal: 7,
          facets,
        }),
      "/library?tag=review&q=alp",
    );
    await screen.findByText("alpha");
    expect(screen.getByText("2 of 7 artifacts")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Remove filter “alp”" }),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Remove filter review" }),
    );
    await waitFor(() => expect(last().filter.tags).toEqual([]));
    expect(last().filter.text).toBe("alp");
    expect(await screen.findByText("7 artifacts")).toBeInTheDocument();
  });

  it("clears one group or all filters", async () => {
    const user = userEvent.setup();
    const { last, search } = setup(
      full,
      "/library?type=skill&tag=review&q=alp",
    );
    await screen.findByText("alpha");
    expect(last().filter).toMatchObject({
      types: ["skill"],
      tags: ["review"],
      text: "alp",
    });
    await user.click(screen.getByRole("button", { name: "Clear type filter" }));
    await waitFor(() => expect(last().filter.types).toEqual([]));
    expect(last().filter.tags).toEqual(["review"]);
    await user.click(screen.getByRole("button", { name: "Clear all" }));
    await waitFor(() => expect(last().filter.tags).toEqual([]));
    expect(last().filter.text).toBeNull();
    expect(search()).toBe("");
    expect(screen.getByLabelText("Search artifacts")).toHaveValue("");
  });

  it("searches after a pause and writes the text to the URL", async () => {
    const user = userEvent.setup();
    const { queries, last, search } = setup(full);
    await screen.findByText("alpha");
    const before = queries.length;
    await user.type(screen.getByLabelText("Search artifacts"), "rev");
    await waitFor(() => expect(last().filter.text).toBe("rev"));
    expect(queries.length).toBe(before + 1);
    expect(search()).toBe("?q=rev");
  });

  it("resets to the first page when a filter changes", async () => {
    const user = userEvent.setup();
    const { last } = setup(
      () => page(sample, { total: 60, facets }),
      "/library?page=2",
    );
    const column = await screen.findByRole("complementary");
    expect(last().offset).toBe(24);
    await user.click(within(column).getByRole("button", { name: /Skills/ }));
    await waitFor(() => expect(last().filter.types).toEqual(["skill"]));
    expect(last().offset).toBe(0);
  });

  it("restores filters and sort on Back", async () => {
    const user = userEvent.setup();
    const { last, search } = setup(full);
    const column = await screen.findByRole("complementary");
    await user.click(within(column).getByRole("button", { name: /Skills/ }));
    await waitFor(() => expect(search()).toBe("?type=skill"));
    await user.click(screen.getByRole("button", { name: "history back" }));
    await waitFor(() => expect(search()).toBe(""));
    await waitFor(() => expect(last().filter.types).toEqual([]));
  });

  it("reads the whole state from the URL", async () => {
    const { last } = setup(
      full,
      "/library?view=list&sort=importedAt&dir=desc&page=2&q=x&type=agent&type=bogus&tag=docs&source=s1",
    );
    await screen.findByRole("table");
    expect(last()).toEqual({
      filter: {
        types: ["agent"],
        tags: ["docs"],
        sources: ["s1"],
        text: "x",
        trustMin: null,
        tiers: [],
        hasFindings: false,
        outdated: false,
        statuses: [],
      },
      sort: "importedAt",
      descending: true,
      offset: 25,
      limit: 25,
    });
  });

  it("ignores junk in the URL", async () => {
    const { last } = setup(full, "/library?sort=nope&page=-3&view=wat");
    await screen.findByText("alpha");
    expect(last()).toMatchObject({ sort: "name", offset: 0, limit: 24 });
  });
});

describe("pager", () => {
  const many = () => page(sample, { total: 60, facets });

  it("moves between pages and keeps the page in the URL", async () => {
    const user = userEvent.setup();
    const { last, search } = setup(many);
    const pager = await screen.findByRole("navigation", { name: "Pages" });
    expect(pager).toHaveTextContent("1–24 of 60");
    expect(
      within(pager).getByRole("button", { name: "Previous page" }),
    ).toBeDisabled();
    await user.click(within(pager).getByRole("button", { name: "Next page" }));
    await waitFor(() => expect(last().offset).toBe(24));
    expect(search()).toBe("?page=2");
    expect(screen.getByRole("navigation", { name: "Pages" })).toHaveTextContent(
      "25–48 of 60",
    );
    await user.click(screen.getByRole("button", { name: "3" }));
    await waitFor(() => expect(last().offset).toBe(48));
    expect(screen.getByRole("navigation", { name: "Pages" })).toHaveTextContent(
      "49–60 of 60",
    );
    expect(screen.getByRole("button", { name: "Next page" })).toBeDisabled();
  });

  it("is absent when everything fits one page", async () => {
    setup(full);
    await screen.findByText("alpha");
    expect(screen.queryByRole("navigation", { name: "Pages" })).toBeNull();
  });

  it("steps back to the last page when the page is past the end", async () => {
    const { last } = setup(
      (q) => (q.offset >= 48 ? page([], { total: 30, facets }) : many()),
      "/library?page=3",
    );
    await waitFor(() => expect(last().offset).toBe(24));
  });
});

describe("states", () => {
  it("shows the welcome for an empty library and leads to import", async () => {
    const user = userEvent.setup();
    setup(() => page([]));
    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: /Your Claude Code/,
      }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("complementary")).toBeNull();
    await user.click(
      screen.getByRole("button", { name: /Import your first artifacts/ }),
    );
    expect(
      await screen.findByRole("heading", { name: "Import screen" }),
    ).toBeInTheDocument();
  });

  it("shows no matches, not the welcome, when filters empty the list", async () => {
    const user = userEvent.setup();
    const { last } = setup(
      (q) => (q.filter.text ? page([], { facets }) : full()),
      "/library?q=zzz",
    );
    expect(await screen.findByText("No artifacts match")).toBeInTheDocument();
    expect(screen.getByText(/these filters and “zzz”/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(last().filter.text).toBeNull());
    expect(await screen.findByText("alpha")).toBeInTheDocument();
  });

  it("shows a loading state first", async () => {
    setup(() => new Promise<Page>(() => undefined));
    expect(await screen.findByRole("status")).toHaveTextContent("Loading");
  });

  it("shows the error and retries", async () => {
    const user = userEvent.setup();
    let calls = 0;
    const { queries } = setup(() => {
      calls += 1;
      if (calls === 1) throw "library is locked";
      return full();
    });
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("library is locked");
    await user.click(within(alert).getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("alpha")).toBeInTheDocument();
    expect(queries).toHaveLength(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
