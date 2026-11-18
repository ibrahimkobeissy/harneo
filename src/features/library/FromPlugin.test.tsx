import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";
import { ItemCard } from "./ItemCard";
import { ItemTable } from "./ItemTable";

const part = item("alpha", { fromPlugin: "toolbox" });

const detail: ArtifactDetail = {
  id: "id-alpha",
  artifact: {
    kind: "skill",
    name: "alpha",
    description: "About alpha",
    path: "skills/alpha",
    files: [],
    linkedFiles: [],
    metadata: {
      fromPlugin: { name: "toolbox", folder: "", marketplace: null },
    },
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
      location: "/home/me/kit",
      branch: null,
      subfolder: null,
      commitSha: null,
    },
    sourcePath: "skills/alpha",
    digest: null,
    importedAt: 1_700_000_000n,
    replacedSecretKeys: [],
  },
  files: ["alpha/SKILL.md"],
};

describe("unpacked parts in the library", () => {
  it("names the plugin on a card and a list row, beside the source", () => {
    const { unmount } = render(<ItemCard item={part} onOpen={() => {}} />);
    expect(screen.getByText(/from plugin toolbox/)).toBeInTheDocument();
    unmount();
    render(
      <ItemTable
        items={[part, item("plain")]}
        onOpen={() => {}}
        sort="name"
        descending={false}
        onSort={() => {}}
      />,
    );
    expect(screen.getAllByText(/from plugin toolbox/)).toHaveLength(1);
  });

  it("names the plugin in the detail panel", async () => {
    mockIPC((cmd) => {
      if (cmd === "library_list") return page([part]);
      if (cmd === "library_detail") return detail;
      return cmd === "tags_list" || cmd === "artifact_tags" ? [] : undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    expect(within(panel).getByText("from plugin toolbox")).toBeInTheDocument();
  });

  it("marks a part with no plugin record Re-import to update, and only that part", async () => {
    const old = item("alpha", {
      fromPlugin: "toolbox",
      reimportToUpdate: true,
    });
    const fresh = item("beta", { fromPlugin: "toolbox" });
    const { unmount } = render(<ItemCard item={fresh} onOpen={() => {}} />);
    expect(screen.queryByText("Re-import to update")).toBeNull();
    unmount();
    render(<ItemCard item={old} onOpen={() => {}} />);
    expect(screen.getByText("Re-import to update")).toBeInTheDocument();
  });

  it("says so in the detail panel, with what happens meanwhile", async () => {
    const old = item("alpha", {
      fromPlugin: "toolbox",
      reimportToUpdate: true,
    });
    mockIPC((cmd) => {
      if (cmd === "library_list") return page([old]);
      if (cmd === "library_detail")
        return { ...detail, reimportToUpdate: true };
      return cmd === "tags_list" || cmd === "artifact_tags" ? [] : undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await userEvent
      .setup()
      .click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    const note = within(panel).getByRole("region", {
      name: "Re-import to update",
    });
    expect(note).toHaveTextContent(/installs as a loose copy/);
    expect(
      within(panel).queryByText("Replace with the whole plugin"),
    ).toBeNull();
  });
});
