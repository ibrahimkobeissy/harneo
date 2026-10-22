import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { LibraryItem } from "../../bindings/LibraryItem";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

function detail(
  name: string,
  over: Partial<ArtifactDetail["artifact"]> = {},
): ArtifactDetail {
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
      ...over,
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
    files: [`${name}/SKILL.md`, `${name}/notes.md`],
  };
}

const alpha = detail("alpha", {
  references: [
    { kind: "agent", target: "beta", resolved: true },
    { kind: "agent", target: "ghost", resolved: true },
    { kind: "command", target: "gone", resolved: false },
  ],
  diagnostics: [
    {
      severity: "warning",
      code: "skill.no-description",
      message: "Add a description",
      file: "alpha/SKILL.md",
    },
    {
      severity: "error",
      code: "skill.bad-frontmatter",
      message: "Frontmatter is broken",
      file: "alpha/SKILL.md",
    },
  ],
});
const beta = detail("beta", { kind: "agent" });

function Probe() {
  return <span data-testid="search">{useLocation().search}</span>;
}

function setup(path = "/library", details: ArtifactDetail[] = [alpha, beta]) {
  let items: LibraryItem[] = [item("alpha"), item("beta", {}, "agent")];
  const removed: string[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, string>;
    switch (cmd) {
      case "library_list":
        return page(items);
      case "library_detail": {
        const found = details.find((d) => d.id === a.id);
        if (!found) throw new Error(`artifact ${a.id} is not in the library`);
        return found;
      }
      case "library_find":
        return a.name === "beta" ? "id-beta" : null;
      case "library_remove":
        removed.push(String(a.id));
        items = items.filter((i) => i.id !== a.id);
        return undefined;
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return { removed };
}

describe("detail panel", () => {
  it("opens from a card with its sections and puts the id in the URL", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    expect(screen.getByTestId("search")).toHaveTextContent("open=id-alpha");
    expect(within(panel).getByText("About alpha")).toBeInTheDocument();
    expect(within(panel).getByText("/home/me/kit")).toBeInTheDocument();
    expect(within(panel).getByText("alpha/notes.md")).toBeInTheDocument();
    expect(
      within(panel).getByText("skill.bad-frontmatter"),
    ).toBeInTheDocument();
    expect(within(panel).getByText("Errors (1)")).toBeInTheDocument();
    expect(within(panel).getByText("Warnings (1)")).toBeInTheDocument();
    expect(within(panel).getByText(/Imported/)).toBeInTheDocument();
  });

  it("shows git provenance with the short commit and the full one in a title", async () => {
    const sha = "0123456789abcdef0123456789abcdef01234567";
    const git = detail("alpha");
    git.provenance.source = {
      id: "s2",
      kind: "git",
      location: "https://example.com/kit",
      branch: "main",
      subfolder: "skills",
      commitSha: sha,
    };
    setup("/library?open=id-alpha", [git]);
    await screen.findByRole("dialog", { name: "alpha details" });
    expect(screen.getByText("https://example.com/kit")).toBeInTheDocument();
    expect(screen.getByText("main")).toBeInTheDocument();
    expect(screen.getByText("skills")).toBeInTheDocument();
    expect(screen.getByText("0123456")).toHaveAttribute("title", sha);
  });

  it("restores the open panel from the URL", async () => {
    setup("/library?open=id-beta");
    expect(
      await screen.findByRole("dialog", { name: "beta details" }),
    ).toBeInTheDocument();
  });

  it("opens a resolved reference, lists others as not in library, and Back returns", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    expect(
      await within(panel).findByRole("button", { name: /beta/ }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(within(panel).getAllByText(/not in library/)).toHaveLength(2),
    );
    expect(
      within(panel).queryByRole("button", { name: /ghost|gone/ }),
    ).not.toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: /beta/ }));
    expect(
      await screen.findByRole("dialog", { name: "beta details" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("search")).toHaveTextContent("open=id-beta");
    await user.click(screen.getByRole("button", { name: "Back to alpha" }));
    expect(
      await screen.findByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Back to/ }),
    ).not.toBeInTheDocument();
  });

  it("opens from a focused card with Enter, Esc closes and focus returns", async () => {
    const user = userEvent.setup();
    setup();
    const card = await screen.findByRole("button", { name: /alpha/ });
    card.focus();
    await user.keyboard("{Enter}");
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await waitFor(() =>
      expect(panel.contains(document.activeElement)).toBe(true),
    );
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId("search")).not.toHaveTextContent("open=");
    expect(screen.getByRole("button", { name: /alpha/ })).toHaveFocus();
  });

  it("asks before removing, then calls IPC and drops the item from the list", async () => {
    const user = userEvent.setup();
    const { removed } = setup("/library?open=id-alpha");
    await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(screen.getByRole("button", { name: "Remove" }));
    const confirm = await screen.findByRole("dialog", { name: "Remove alpha" });
    expect(removed).toEqual([]);
    // Esc cancels the confirmation only, not the panel.
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Remove alpha" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      screen.getByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    const again = await screen.findByRole("dialog", { name: "Remove alpha" });
    expect(confirm).not.toBe(again);
    await user.click(within(again).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(removed).toEqual(["id-alpha"]));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: /alpha/ }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: /beta/ })).toBeInTheDocument();
  });

  it("shows the error when the artifact cannot be loaded", async () => {
    setup("/library?open=id-missing");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "not in the library",
    );
  });
});
