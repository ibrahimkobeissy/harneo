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
      origin: { type: "own" },
      ...over,
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
      case "tags_list":
      case "artifact_tags":
        return [];
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
    expect(
      within(panel).getByRole("treeitem", { name: /alpha\// }),
    ).toBeInTheDocument();
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
    await user.click(
      screen.getByRole("button", { name: "Delete from library…" }),
    );
    const confirm = await screen.findByRole("dialog", { name: "Delete alpha" });
    expect(removed).toEqual([]);
    // Esc cancels the confirmation only, not the panel.
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Delete alpha" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      screen.getByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Delete from library…" }),
    );
    const again = await screen.findByRole("dialog", { name: "Delete alpha" });
    expect(confirm).not.toBe(again);
    await user.click(within(again).getByRole("button", { name: "Delete" }));
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

describe("detail panel files", () => {
  it("opens a file in the viewer and returns focus to its row on close", async () => {
    const user = userEvent.setup();
    mockIPC((cmd, args) => {
      const a = (args ?? {}) as Record<string, string>;
      if (cmd === "library_list") return page([item("alpha")]);
      if (cmd === "library_detail") return alpha;
      if (cmd === "library_read_file")
        return { kind: "text", text: `# ${a.path}` };
      if (cmd === "tags_list" || cmd === "artifact_tags") return [];
      return undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await user.click(await screen.findByRole("treeitem", { name: /alpha\// }));
    const row = screen.getByRole("treeitem", { name: "SKILL.md" });
    await user.click(row);
    expect(
      await screen.findByRole("dialog", { name: "alpha/SKILL.md" }),
    ).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "alpha/SKILL.md" }),
      ).not.toBeInTheDocument(),
    );
    expect(
      screen.getByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();
    expect(row).toHaveFocus();
  });

  it("opens a file in the default editor from its row and shows a refusal", async () => {
    const user = userEvent.setup();
    const opened: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "library_list") return page([item("alpha")]);
      if (cmd === "library_detail") return alpha;
      if (cmd === "library_open_file") {
        opened.push(args);
        if ((args as { path: string }).path.endsWith("notes.md"))
          throw new Error("nope");
        return undefined;
      }
      if (cmd === "tags_list" || cmd === "artifact_tags") return [];
      return undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await user.click(await screen.findByRole("treeitem", { name: /alpha\// }));
    await user.click(
      await screen.findByRole("button", {
        name: "Open alpha/SKILL.md in editor",
      }),
    );
    expect(opened[0]).toEqual({ id: "id-alpha", path: "alpha/SKILL.md" });
    await user.click(
      screen.getByRole("button", { name: "Open alpha/notes.md in editor" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("nope");
  });

  it("lists files as a tree, folders collapsed with counts, and opens a file in the viewer", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "library_list") return page([item("alpha")]);
      if (cmd === "library_detail") return alpha;
      if (cmd === "library_read_file") return { kind: "text", text: "hello" };
      if (cmd === "tags_list" || cmd === "artifact_tags") return [];
      return undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    const tree = await screen.findByRole("tree", { name: "Files" });
    const folder = within(tree).getByRole("treeitem", { name: /alpha\// });
    expect(folder).toHaveAttribute("aria-expanded", "false");
    expect(folder).toHaveTextContent("2");
    expect(within(tree).queryByText("SKILL.md")).not.toBeInTheDocument();
    folder.focus();
    await user.keyboard("{ArrowRight}");
    expect(folder).toHaveAttribute("aria-expanded", "true");
    expect(within(tree).getByText("SKILL.md")).toBeInTheDocument();
    await user.keyboard("{ArrowLeft}");
    expect(folder).toHaveAttribute("aria-expanded", "false");
    await user.keyboard("{Enter}");
    await user.click(within(tree).getByText("notes.md"));
    expect(await screen.findByText("hello")).toBeInTheDocument();
  });
});

describe("detail panel width", () => {
  const key = "harneo.detail.width";
  const sep = () => screen.getByRole("separator", { name: "Resize details" });
  const panel = () => screen.getByRole("dialog", { name: "alpha details" });
  const open = async () => {
    setup("/library?open=id-alpha");
    await screen.findByRole("dialog", { name: "alpha details" });
  };

  beforeEach(() => {
    window.localStorage.clear();
    vi.stubGlobal("innerWidth", 1600);
    // jsdom has no pointer capture.
    const proto = HTMLElement.prototype;
    proto.setPointerCapture = vi.fn();
    proto.releasePointerCapture = vi.fn();
    proto.hasPointerCapture = vi.fn(() => true);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("exposes its value and limits", async () => {
    await open();
    expect(sep()).toHaveAttribute("aria-orientation", "vertical");
    expect(sep()).toHaveAttribute("aria-valuenow", "460");
    expect(sep()).toHaveAttribute("aria-valuemin", "460");
    expect(sep()).toHaveAttribute("aria-valuemax", "960");
    expect(sep()).toHaveAttribute("tabindex", "0");
  });

  it("grows with ArrowLeft, shrinks with ArrowRight, jumps with Home and End", async () => {
    const user = userEvent.setup();
    await open();
    sep().focus();
    await user.keyboard("{ArrowLeft}");
    expect(sep()).toHaveAttribute("aria-valuenow", "476");
    expect(panel().style.width).toBe("476px");
    expect(window.localStorage.getItem(key)).toBe("476");
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(sep()).toHaveAttribute("aria-valuenow", "460");
    await user.keyboard("{End}");
    expect(sep()).toHaveAttribute("aria-valuenow", "960");
    await user.keyboard("{ArrowLeft}");
    expect(sep()).toHaveAttribute("aria-valuenow", "960");
    await user.keyboard("{Home}");
    expect(sep()).toHaveAttribute("aria-valuenow", "460");
  });

  it("drags the left edge within the limits", async () => {
    await open();
    fireEvent.pointerDown(sep(), { pointerId: 1, clientX: 1140 });
    fireEvent.pointerMove(sep(), { pointerId: 1, clientX: 1040 });
    expect(sep()).toHaveAttribute("aria-valuenow", "560");
    fireEvent.pointerMove(sep(), { pointerId: 1, clientX: 0 });
    expect(sep()).toHaveAttribute("aria-valuenow", "960");
    fireEvent.pointerMove(sep(), { pointerId: 1, clientX: 1600 });
    expect(sep()).toHaveAttribute("aria-valuenow", "460");
    fireEvent.pointerUp(sep(), { pointerId: 1 });
  });

  it("restores a remembered width and clamps it on a smaller window", async () => {
    window.localStorage.setItem(key, "900");
    await open();
    expect(sep()).toHaveAttribute("aria-valuenow", "900");
    act(() => {
      vi.stubGlobal("innerWidth", 1000);
      window.dispatchEvent(new Event("resize"));
    });
    expect(sep()).toHaveAttribute("aria-valuemax", "600");
    expect(sep()).toHaveAttribute("aria-valuenow", "600");
    expect(window.localStorage.getItem(key)).toBe("900");
  });

  it("resets on double-click", async () => {
    const user = userEvent.setup();
    await open();
    sep().focus();
    await user.keyboard("{End}");
    await user.dblClick(sep());
    expect(sep()).toHaveAttribute("aria-valuenow", "460");
  });
});
