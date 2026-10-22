import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Collection } from "../../bindings/Collection";
import type { Filter } from "../../bindings/Filter";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Query } from "../../bindings/Query";
import { CollectionsProvider } from "../../app/collections";
import { Sidebar } from "../../app/Sidebar";
import { item, page } from "../../test/library";
import { CollectionScreen, LibraryScreen, NewCollectionHost } from ".";

const items: LibraryItem[] = [
  item("alpha"),
  item("beta", {}, "agent"),
  item("gamma"),
];

function detailOf(id: string): ArtifactDetail {
  const name = id.replace("id-", "");
  return {
    id,
    artifact: {
      kind: "skill",
      name,
      description: `About ${name}`,
      path: `skills/${name}`,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    provenance: {
      source: {
        id: "s1",
        kind: "local",
        location: "/home/me/kit",
        branch: null,
        subfolder: null,
        commitSha: null,
      },
      sourcePath: `skills/${name}`,
      digest: null,
      importedAt: 1_700_000_000n,
      replacedSecretKeys: [],
    },
    files: [],
  };
}

interface Stored {
  id: number;
  name: string;
  kind: "manual" | "smart";
  rule: Filter | null;
  members: string[];
}

const facets = {
  kinds: [
    { kind: "skill" as const, count: 2 },
    { kind: "agent" as const, count: 1 },
  ],
  tags: [{ name: "review", color: "violet", count: 1 }],
  sources: [
    {
      source: { id: "s1", kind: "local", label: "agent-skills", branch: null },
      count: 3,
    },
  ],
};

function matching(rule: Filter): LibraryItem[] {
  return items.filter(
    (i) => rule.types.length === 0 || rule.types.includes(i.kind),
  );
}

function Probe() {
  return <span data-testid="where">{useLocation().pathname}</span>;
}

/** A fake backend that keeps collections in memory, and the app around the screens. */
function setup(seed: Stored[] = [], path = "/library") {
  const stored = seed.map((c) => structuredClone(c));
  let nextId = 10;
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  const pageQueries: Query[] = [];
  const view = (c: Stored): Collection => ({
    id: BigInt(c.id),
    name: c.name,
    kind: c.kind,
    rule: c.rule,
    count: c.rule ? matching(c.rule).length : c.members.length,
  });
  const find = (id: unknown) => {
    const found = stored.find((c) => c.id === Number(id));
    if (!found) throw new Error(`no collection ${String(id)}`);
    return found;
  };
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    switch (cmd) {
      case "collections_list":
        return stored.map(view);
      case "collection_create_manual":
      case "collection_create_smart": {
        const made: Stored = {
          id: nextId++,
          name: String(a.name),
          kind: cmd === "collection_create_smart" ? "smart" : "manual",
          rule: (a.rule as Filter | undefined) ?? null,
          members: [],
        };
        stored.push(made);
        return view(made);
      }
      case "collection_rename":
        if (a.name === "Taken")
          throw new Error("a collection named Taken exists");
        find(a.id).name = String(a.name);
        return view(find(a.id));
      case "collection_update_rule":
        find(a.id).rule = a.rule as Filter;
        return view(find(a.id));
      case "collection_delete":
        if (find(a.id).name === "Locked")
          throw new Error("cannot delete Locked");
        stored.splice(stored.indexOf(find(a.id)), 1);
        return undefined;
      case "collection_add": {
        const c = find(a.collectionId);
        if (!c.members.includes(String(a.artifactId)))
          c.members.push(String(a.artifactId));
        return undefined;
      }
      case "collection_remove": {
        const c = find(a.collectionId);
        c.members = c.members.filter((m) => m !== a.artifactId);
        return undefined;
      }
      case "artifact_collections":
        return stored
          .filter((c) => c.members.includes(String(a.id)))
          .map((c) => c.id);
      case "collection_page": {
        pageQueries.push(a.query as Query);
        const c = find(a.collectionId);
        const rows = c.rule
          ? matching(c.rule)
          : c.members.flatMap((m) => items.filter((i) => i.id === m));
        return page(rows, { facets });
      }
      case "library_list": {
        const query = a.query as Query;
        return page(matching(query.filter), { facets });
      }
      case "library_detail":
        return detailOf(String(a.id));
      case "tags_list":
        return [{ id: 1, name: "review", color: "violet", count: 1 }];
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <CollectionsProvider>
        <Sidebar
          onShortcuts={() => {}}
          onSearch={() => {}}
          onCollapse={() => {}}
        />
        <Probe />
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
          <Route path="/collections/:id" element={<CollectionScreen />} />
        </Routes>
        <NewCollectionHost />
      </CollectionsProvider>
    </MemoryRouter>,
  );
  const where = () => screen.getByTestId("where").textContent;
  const sent = (cmd: string) => calls.filter((c) => c.cmd === cmd);
  return { stored, calls, pageQueries, where, sent };
}

const manualKit: Stored = {
  id: 1,
  name: "Review kit",
  kind: "manual",
  rule: null,
  members: ["id-alpha", "id-gamma"],
};
const smartSkills: Stored = {
  id: 2,
  name: "All skills",
  kind: "smart",
  rule: { types: ["skill"], tags: [], sources: [], text: null },
  members: [],
};

describe("sidebar", () => {
  it("keeps the empty copy when there are no collections", async () => {
    setup();
    expect(
      await screen.findByText(/None yet\. Saved groupings/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "New collection" }),
    ).toBeInTheDocument();
  });

  it("lists collections with their counts and links to their screens", async () => {
    setup([manualKit, smartSkills]);
    const link = await screen.findByRole("link", { name: "Review kit" });
    expect(link).toHaveAttribute("href", "/collections/1");
    expect(link).toHaveTextContent("2");
    expect(screen.getByRole("link", { name: "All skills" })).toHaveTextContent(
      "2",
    );
    expect(screen.queryByText(/None yet/)).not.toBeInTheDocument();
  });
});

describe("new collection", () => {
  it("creates a manual collection, lists it in the sidebar and opens its screen", async () => {
    const user = userEvent.setup();
    const { where, sent } = setup();
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    const dialog = screen.getByRole("dialog", { name: "New collection" });
    expect(
      within(dialog).getByRole("radio", { name: /Manual: a list you pick/ }),
    ).toBeChecked();
    expect(
      within(dialog).getByText(/Example: “My review kit”/),
    ).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Name"), "Daily");
    await user.click(
      within(dialog).getByRole("button", { name: "Create collection" }),
    );
    expect(await screen.findByRole("link", { name: "Daily" })).toHaveAttribute(
      "href",
      "/collections/10",
    );
    expect(sent("collection_create_manual")[0]?.args).toEqual({
      name: "Daily",
    });
    await waitFor(() => expect(where()).toBe("/collections/10"));
    expect(
      await screen.findByRole("heading", { level: 1, name: /Daily/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Nothing in this collection yet"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("explains that collections install nothing in the empty state", async () => {
    const empty: Stored = { ...manualKit, id: 3, name: "Empty", members: [] };
    setup([empty], "/collections/3");
    expect(
      await screen.findByText(/Collections only organize the library/),
    ).toBeInTheDocument();
  });

  it("builds a smart rule with a live count and creates it", async () => {
    const user = userEvent.setup();
    const { sent, where } = setup();
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    const dialog = screen.getByRole("dialog", { name: "New collection" });
    await user.click(
      within(dialog).getByRole("radio", { name: /Smart: a saved rule/ }),
    );
    expect(
      within(dialog).getByText(/Pick at least one condition/),
    ).toBeInTheDocument();
    // The Tag and Source chips come from tags_list and the library facets.
    expect(
      await within(dialog).findByRole("button", { name: "review" }),
    ).toBeInTheDocument();
    expect(
      await within(dialog).findByRole("button", { name: "agent-skills" }),
    ).toBeInTheDocument();

    // Creating with no condition is refused.
    await user.type(within(dialog).getByLabelText("Name"), "Skills only");
    await user.click(
      within(dialog).getByRole("button", { name: "Create collection" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "Add at least one condition",
    );
    expect(sent("collection_create_smart")).toHaveLength(0);

    await user.click(within(dialog).getByRole("button", { name: "Skills" }));
    expect(
      await within(dialog).findByText(/Matches 2 artifacts now/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Subagents" }));
    expect(
      await within(dialog).findByText(/Matches 3 artifacts now/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Subagents" }));
    await user.click(
      within(dialog).getByRole("button", { name: "Create collection" }),
    );

    expect(
      await screen.findByRole("link", { name: "Skills only" }),
    ).toHaveTextContent("2");
    expect(sent("collection_create_smart")[0]?.args).toEqual({
      name: "Skills only",
      rule: { types: ["skill"], tags: [], sources: [], text: null },
    });
    await waitFor(() => expect(where()).toBe("/collections/10"));
    // The screen shows the members and the rule as chips.
    expect(await screen.findByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("gamma")).toBeInTheDocument();
    expect(screen.queryByText("beta")).not.toBeInTheDocument();
    const rule = screen.getByRole("list", { name: "Rule" });
    expect(within(rule).getByText("Skills")).toBeInTheDocument();
  });

  it("counts with the text condition too", async () => {
    const user = userEvent.setup();
    const { calls } = setup();
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    const dialog = screen.getByRole("dialog", { name: "New collection" });
    await user.click(
      within(dialog).getByRole("radio", { name: /Smart: a saved rule/ }),
    );
    await user.type(within(dialog).getByLabelText("Text"), "review");
    expect(
      await within(dialog).findByText(/Matches 3 artifacts now/),
    ).toBeInTheDocument();
    const asked = calls
      .filter((c) => c.cmd === "library_list")
      .map((c) => (c.args.query as Query).filter.text);
    expect(asked).toContain("review");
  });

  it("asks for a name and focuses the field instead of using a default", async () => {
    const user = userEvent.setup();
    const { sent } = setup();
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    await user.click(screen.getByRole("button", { name: "Create collection" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Give the collection a name",
    );
    const field = screen.getByRole("textbox", { name: "Name" });
    expect(field).toHaveFocus();
    expect(field).toBeInvalid();
    expect(sent("collection_create_manual")).toHaveLength(0);
  });

  it("closes when the route changes", async () => {
    const user = userEvent.setup();
    setup([manualKit]);
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    expect(
      screen.getByRole("dialog", { name: "New collection" }),
    ).toBeInTheDocument();
    await user.click(await screen.findByRole("link", { name: /Review kit/ }));
    expect(
      screen.queryByRole("dialog", { name: "New collection" }),
    ).not.toBeInTheDocument();
  });

  it("reports a name the library refuses", async () => {
    const user = userEvent.setup();
    setup();
    mockIPC((cmd) => {
      if (cmd === "collection_create_manual") throw new Error("name taken");
      return cmd === "collections_list" ? [] : undefined;
    });
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Taken");
    await user.click(screen.getByRole("button", { name: "Create collection" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("name taken");
    expect(
      screen.getByRole("dialog", { name: "New collection" }),
    ).toBeInTheDocument();
  });

  it("opens pre-filled from the library's filters", async () => {
    const user = userEvent.setup();
    const { sent } = setup([], "/library?type=skill&tag=review");
    await user.click(
      await screen.findByRole("button", { name: /Save as smart collection/ }),
    );
    const dialog = screen.getByRole("dialog", { name: "New collection" });
    expect(
      within(dialog).getByRole("radio", { name: /Smart: a saved rule/ }),
    ).toBeChecked();
    expect(
      within(dialog).getByText(/pre-filled from your current filters/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("button", { name: "Skills" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      await within(dialog).findByRole("button", { name: "review" }),
    ).toHaveAttribute("aria-pressed", "true");
    await user.type(within(dialog).getByLabelText("Name"), "Reviewing");
    await user.click(
      within(dialog).getByRole("button", { name: "Create collection" }),
    );
    await screen.findByRole("link", { name: "Reviewing" });
    expect(sent("collection_create_smart")[0]?.args).toEqual({
      name: "Reviewing",
      rule: { types: ["skill"], tags: ["review"], sources: [], text: null },
    });
  });

  it("closes on Cancel and Esc without creating anything", async () => {
    const user = userEvent.setup();
    const { sent } = setup();
    await user.click(
      await screen.findByRole("button", { name: "New collection" }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "New collection" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(sent("collection_create_manual")).toHaveLength(0);
  });
});

describe("collection screen", () => {
  it("lists a manual collection's members in manual order through collection_page", async () => {
    const { pageQueries } = setup([manualKit], "/collections/1");
    expect(await screen.findByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("gamma")).toBeInTheDocument();
    expect(screen.queryByText("beta")).not.toBeInTheDocument();
    expect(pageQueries[0]).toMatchObject({
      sort: "position",
      descending: false,
    });
    expect(screen.getByText("Manual")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Manual order/ }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/Hand-picked/)).toBeInTheDocument();
    // The "only organize" note belongs to the empty state, not to every collection.
    expect(screen.queryByText(/only organize/)).not.toBeInTheDocument();
    // The filter column is the library's, and a manual collection has no rule to edit.
    expect(
      screen.getByRole("complementary", { name: "Filters" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Edit rule" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Save as smart collection/ }),
    ).not.toBeInTheDocument();
  });

  it("sorts a smart collection by name and offers no manual order", async () => {
    const { pageQueries } = setup([smartSkills], "/collections/2");
    expect(await screen.findByText("alpha")).toBeInTheDocument();
    expect(pageQueries[0]).toMatchObject({ sort: "name" });
    expect(
      screen.queryByRole("button", { name: /Manual order/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Smart")).toBeInTheDocument();
  });

  it("says when a collection is gone", async () => {
    setup([], "/collections/99");
    expect(
      await screen.findByText("This collection is gone"),
    ).toBeInTheDocument();
  });

  it("takes an artifact out of a manual collection from the detail panel", async () => {
    const user = userEvent.setup();
    const { sent } = setup([manualKit], "/collections/1");
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      within(panel).getByRole("button", { name: "Remove from collection" }),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "alpha details" }),
      ).not.toBeInTheDocument(),
    );
    expect(sent("collection_remove")[0]?.args).toEqual({
      collectionId: 1,
      artifactId: "id-alpha",
    });
    expect(screen.getByRole("status")).toHaveTextContent(
      "Removed from Review kit",
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: /alpha/ }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByText("gamma")).toBeInTheDocument();
    // The sidebar count follows.
    await waitFor(() =>
      expect(
        screen.getByRole("link", { name: "Review kit" }),
      ).toHaveTextContent("1"),
    );
  });

  it("offers no Remove from collection on a smart collection", async () => {
    const user = userEvent.setup();
    setup([smartSkills], "/collections/2");
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await within(panel).findByRole("button", { name: /Add to/ });
    expect(
      within(panel).queryByRole("button", { name: "Remove from collection" }),
    ).not.toBeInTheDocument();
  });

  it("renames the collection", async () => {
    const user = userEvent.setup();
    const { sent } = setup([manualKit], "/collections/1");
    await user.click(await screen.findByRole("button", { name: "Rename" }));
    const dialog = screen.getByRole("dialog", { name: "Rename collection" });
    const name = within(dialog).getByLabelText("Name");
    expect(name).toHaveValue("Review kit");
    await user.clear(name);
    await user.type(name, "Kit v2");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(
      await screen.findByRole("link", { name: "Kit v2" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("heading", { level: 1, name: /Kit v2/ }),
    ).toBeInTheDocument();
    expect(sent("collection_rename")[0]?.args).toEqual({
      id: 1,
      name: "Kit v2",
    });
  });

  it("keeps the rename dialog open with the error when the name is taken", async () => {
    const user = userEvent.setup();
    setup([manualKit], "/collections/1");
    await user.click(await screen.findByRole("button", { name: "Rename" }));
    const dialog = screen.getByRole("dialog", { name: "Rename collection" });
    const name = within(dialog).getByLabelText("Name");
    await user.clear(name);
    await user.type(name, "Taken");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await within(dialog).findByText(/Taken exists/)).toBeInTheDocument();
    expect(
      screen.getByRole("dialog", { name: "Rename collection" }),
    ).toBeInTheDocument();
  });

  it("shows a delete failure and stays on the collection", async () => {
    const user = userEvent.setup();
    const { where } = setup(
      [{ ...manualKit, name: "Locked" }],
      "/collections/1",
    );
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Delete collection",
      }),
    );
    expect(await screen.findByText(/cannot delete Locked/)).toBeInTheDocument();
    expect(where()).toBe("/collections/1");
  });

  it("names the sources of a smart rule, and a missing one generically", async () => {
    setup(
      [
        {
          ...smartSkills,
          rule: { types: [], tags: [], sources: ["s1", "zz"], text: null },
        },
      ],
      "/collections/2",
    );
    const rule = await screen.findByRole("list", { name: "Rule" });
    expect(await within(rule).findByText("agent-skills")).toBeInTheDocument();
    expect(within(rule).getByText("a source")).toBeInTheDocument();
  });

  it("edits a smart collection's rule and reloads its members", async () => {
    const user = userEvent.setup();
    const { sent, pageQueries } = setup([smartSkills], "/collections/2");
    await screen.findByText("alpha");
    expect(screen.queryByText("beta")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit rule" }));
    const dialog = screen.getByRole("dialog", { name: "Edit rule" });
    expect(
      within(dialog).getByRole("button", { name: "Skills" }),
    ).toHaveAttribute("aria-pressed", "true");
    await user.click(within(dialog).getByRole("button", { name: "Skills" }));
    await user.click(within(dialog).getByRole("button", { name: "Subagents" }));
    await user.click(within(dialog).getByRole("button", { name: "Save rule" }));
    expect(await screen.findByText("beta")).toBeInTheDocument();
    expect(sent("collection_update_rule")[0]?.args).toEqual({
      id: 2,
      rule: { types: ["agent"], tags: [], sources: [], text: null },
    });
    expect(pageQueries.length).toBeGreaterThan(1);
  });

  it("asks before deleting, keeps the collection on Cancel, and goes to the library after", async () => {
    const user = userEvent.setup();
    const { sent, where, stored } = setup([manualKit], "/collections/1");
    await user.click(await screen.findByRole("button", { name: "Delete" }));
    const dialog = screen.getByRole("dialog", { name: "Delete Review kit" });
    expect(dialog).toHaveTextContent("The artifacts stay in your library");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(sent("collection_delete")).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Delete collection",
      }),
    );
    await waitFor(() => expect(where()).toBe("/library"));
    expect(sent("collection_delete")[0]?.args).toEqual({ id: 1 });
    expect(stored).toHaveLength(0);
    await waitFor(() =>
      expect(
        screen.queryByRole("link", { name: "Review kit" }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByText(/None yet/)).toBeInTheDocument();
  });
});

describe("Add to… in the detail panel", () => {
  it("lists manual collections, checks the ones holding the artifact, and toggles membership", async () => {
    const user = userEvent.setup();
    const empty: Stored = { ...manualKit, id: 3, name: "Empty", members: [] };
    const { sent } = setup([manualKit, smartSkills, empty], "/library");
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add to/ }),
    );
    const menu = within(panel).getByRole("menu", { name: "Add to" });
    expect(
      within(menu).queryByRole("menuitemcheckbox", { name: /All skills/ }),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(
        within(menu).getByRole("menuitemcheckbox", { name: /Review kit/ }),
      ).toHaveAttribute("aria-checked", "true"),
    );
    expect(
      within(menu).getByRole("menuitemcheckbox", { name: /Empty/ }),
    ).toHaveAttribute("aria-checked", "false");

    await user.click(
      within(menu).getByRole("menuitemcheckbox", { name: /Empty/ }),
    );
    await waitFor(() =>
      expect(
        within(menu).getByRole("menuitemcheckbox", { name: /Empty/ }),
      ).toHaveAttribute("aria-checked", "true"),
    );
    expect(sent("collection_add")[0]?.args).toEqual({
      collectionId: 3,
      artifactId: "id-alpha",
    });
    await waitFor(() =>
      expect(screen.getByRole("link", { name: "Empty" })).toHaveTextContent(
        "1",
      ),
    );

    await user.click(
      within(menu).getByRole("menuitemcheckbox", { name: /Review kit/ }),
    );
    await waitFor(() =>
      expect(
        within(menu).getByRole("menuitemcheckbox", { name: /Review kit/ }),
      ).toHaveAttribute("aria-checked", "false"),
    );
    expect(sent("collection_remove")[0]?.args).toEqual({
      collectionId: 1,
      artifactId: "id-alpha",
    });
  });

  it("closes the menu on Esc without closing the panel, and opens New collection…", async () => {
    const user = userEvent.setup();
    setup([manualKit], "/library");
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add to/ }),
    );
    await user.keyboard("{Escape}");
    expect(within(panel).queryByRole("menu")).not.toBeInTheDocument();
    expect(
      screen.getByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();

    await user.click(within(panel).getByRole("button", { name: /Add to/ }));
    await user.click(
      within(panel).getByRole("menuitem", { name: /New collection/ }),
    );
    expect(
      screen.getByRole("dialog", { name: "New collection" }),
    ).toBeInTheDocument();
  });
});
