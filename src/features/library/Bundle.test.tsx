import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Links } from "../../bindings/Links";
import type { LinkView } from "../../bindings/LinkView";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

function detail(name: string, links: Links): ArtifactDetail {
  return {
    id: `id-${name}`,
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
    audit: null,
    auditedAt: null,
    links,
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

function view(name: string, origin: LinkView["origin"], reason = ""): LinkView {
  return { id: `id-${name}`, kind: "agent", name, origin, reason };
}

function setup(
  path = "/library",
  failAdd = false,
  start: { needs?: LinkView[]; neededBy?: LinkView[] } = {},
) {
  let alphaNeeds: LinkView[] = start.needs ?? [
    view("beta", "reference"),
    view("gamma", "ai", "Calls gamma for review"),
    view("delta", "manual"),
  ];
  const alphaNeededBy = start.neededBy ?? [];
  const calls: string[] = [];
  const searches: string[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, string>;
    switch (cmd) {
      case "library_list": {
        const text = (
          a as unknown as { query: { filter: { text: string | null } } }
        ).query.filter.text;
        searches.push(text ?? "");
        return page(
          [
            item("alpha", { bundleSize: 3 }),
            item("beta", {}, "agent"),
            item("gamma", {}, "agent"),
            item("delta", {}, "agent"),
            item("epsilon", {}, "agent"),
            item("zeta", {}, "agent"),
          ].filter((i) => !text || i.name.includes(text)),
        );
      }
      case "library_detail":
        if (a.id === "id-alpha")
          return detail("alpha", {
            needs: alphaNeeds,
            neededBy: alphaNeededBy,
          });
        return detail(String(a.id).replace("id-", ""), {
          needs: [],
          neededBy: [view("alpha", "reference")],
        });
      case "links_of":
        return { needs: alphaNeeds, neededBy: alphaNeededBy };
      case "link_remove":
        calls.push(`remove ${a.from}>${a.to}`);
        alphaNeeds = alphaNeeds.filter((l) => l.id !== a.to);
        return undefined;
      case "link_add":
        if (failAdd) throw new Error("no such artifact");
        calls.push(`add ${a.from}>${a.to} ${a.origin}`);
        alphaNeeds = [
          ...alphaNeeds,
          view(String(a.to).replace("id-", ""), "manual"),
        ];
        return undefined;
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls, searches };
}

describe("bundles in the library", () => {
  it("shows Bundle of N on the card only when N is above zero", async () => {
    setup();
    expect(await screen.findByText("Bundle of 3")).toBeInTheDocument();
    expect(screen.getAllByText(/Bundle of/)).toHaveLength(1);
  });

  it("shows Bundle of N beside the status in the list view, with no column of its own", async () => {
    setup("/library?view=list");
    expect(await screen.findByText("Bundle of 3")).toBeInTheDocument();
    expect(
      screen.queryByRole("columnheader", { name: "Bundle" }),
    ).not.toBeInTheDocument();
  });

  it("lists needs with origin chips and explains reference links", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    const needs = await within(panel).findByRole("list", { name: "Needs" });
    expect(within(needs).getByText("Reference")).toBeInTheDocument();
    expect(within(needs).getByText("Claude")).toBeInTheDocument();
    expect(within(needs).getByText("Manual")).toBeInTheDocument();
    expect(
      within(panel).getByText("Calls gamma for review"),
    ).toBeInTheDocument();
    expect(
      within(panel).getByText(/cannot be removed here/),
    ).toBeInTheDocument();
    // Reference links have no remove button.
    expect(
      within(panel).queryByRole("button", { name: /Remove link: needs beta/ }),
    ).toBeNull();
    expect(
      within(panel).getByRole("button", { name: /Remove link: needs gamma/ }),
    ).toBeInTheDocument();
    await user.click(within(needs).getByRole("button", { name: /beta/ }));
    const beta = await screen.findByRole("dialog", { name: "beta details" });
    expect(
      within(beta).getByRole("button", { name: /Back to alpha/ }),
    ).toBeInTheDocument();
    expect(
      await within(beta).findByRole("list", { name: "Needed by" }),
    ).toBeInTheDocument();
  });

  it("removes a manual link", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library?open=id-alpha");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", {
        name: /Remove link: needs delta/,
      }),
    );
    await waitFor(() => expect(calls).toEqual(["remove id-alpha>id-delta"]));
    await waitFor(() =>
      expect(
        within(panel).queryByRole("button", { name: /needs delta/ }),
      ).toBeNull(),
    );
  });

  it("adds a link from the picker and leaves out what is already needed", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library?open=id-alpha");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add a link/ }),
    );
    const list = await within(panel).findByRole("listbox", {
      name: "Artifacts",
    });
    await waitFor(() =>
      expect(within(list).getAllByRole("option")).toHaveLength(2),
    );
    expect(within(list).queryByText("beta")).toBeNull();
    expect(within(list).queryByText("alpha")).toBeNull();
    await user.click(within(list).getByRole("option", { name: /epsilon/ }));
    await waitFor(() =>
      expect(calls).toEqual(["add id-alpha>id-epsilon manual"]),
    );
    const needs = await within(panel).findByRole("list", { name: "Needs" });
    await waitFor(() =>
      expect(within(needs).getByText("epsilon")).toBeInTheDocument(),
    );
  });

  it("picks with the keyboard, searches by name and closes on Escape", async () => {
    const user = userEvent.setup();
    const { calls, searches } = setup("/library?open=id-alpha");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add a link/ }),
    );
    const box = await within(panel).findByRole("combobox");
    await user.type(box, "ze");
    await waitFor(() => expect(searches).toContain("ze"));
    await waitFor(() =>
      expect(within(panel).getAllByRole("option")).toHaveLength(1),
    );
    await user.keyboard("{Escape}");
    expect(within(panel).queryByRole("combobox")).toBeNull();
    // Escape closed the picker only, not the panel.
    expect(screen.getByRole("dialog", { name: "alpha details" })).toBeVisible();
    await user.click(within(panel).getByRole("button", { name: /Add a link/ }));
    const again = await within(panel).findByRole("combobox");
    await waitFor(() =>
      expect(within(panel).getAllByRole("option")).toHaveLength(2),
    );
    await user.click(again);
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}{Enter}");
    await waitFor(() => expect(calls).toContain("add id-alpha>id-zeta manual"));
  });

  it("says when nothing matches and reports a failed add", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha", true);
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add a link/ }),
    );
    const box = await within(panel).findByRole("combobox");
    await user.type(box, "nomatch");
    expect(await within(panel).findByText(/No other artifact/)).toBeVisible();
    await user.clear(box);
    await user.click(
      await within(panel).findByRole("option", { name: /epsilon/ }),
    );
    expect(await within(panel).findByRole("alert")).toHaveTextContent(
      "no such artifact",
    );
  });

  it("shows one line and Add a link when there are no links", async () => {
    setup("/library?open=id-alpha", false, { needs: [] });
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    expect(
      await within(panel).findByText("Needs nothing, nothing needs it."),
    ).toBeVisible();
    expect(within(panel).queryByRole("heading", { name: /^Needs/ })).toBeNull();
    expect(
      within(panel).getByRole("button", { name: /Add a link/ }),
    ).toBeVisible();
  });

  it("leaves what needs it out of the picker", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha", false, {
      neededBy: [view("zeta", "manual")],
    });
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: /Add a link/ }),
    );
    const list = await within(panel).findByRole("listbox", {
      name: "Artifacts",
    });
    await waitFor(() =>
      expect(within(list).getAllByRole("option")).toHaveLength(1),
    );
    expect(within(list).queryByText("zeta")).toBeNull();
  });
});
