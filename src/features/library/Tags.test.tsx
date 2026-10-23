import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { TagColor } from "../../bindings/TagColor";
import { tagDelete, tagRecolor, tagRename } from "../../lib/ipc";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const detail: ArtifactDetail = {
  id: "id-alpha",
  artifact: {
    kind: "skill",
    name: "alpha",
    description: "About alpha",
    path: "skills/alpha",
    files: [],
    metadata: {},
    references: [],
    diagnostics: [],
    digest: null,
    origin: { type: "own" },
  },
  audit: null,
  auditedAt: null,
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

interface Row {
  id: number;
  name: string;
  color: TagColor;
}

function setup(existing: Row[], onArtifact: number[] = []) {
  const tags = [...existing];
  let mine = [...onArtifact];
  let nextId = 100;
  const calls: string[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "library_list": {
        const chips = tags
          .filter((t) => mine.includes(t.id))
          .map((t) => ({ name: t.name, color: t.color }));
        return page([item("alpha", { tags: chips })], {
          facets: {
            tiers: [],
            withFindings: 0,
            kinds: [],
            sources: [],
            tags: tags
              .filter((t) => mine.includes(t.id))
              .map((t) => ({ name: t.name, color: t.color, count: 1 })),
          },
        });
      }
      case "library_detail":
        return detail;
      case "library_find":
        return null;
      case "tags_list":
        return tags.map((t) => ({
          ...t,
          count: mine.includes(t.id) ? 1 : 0,
        }));
      case "artifact_tags":
        return tags.filter((t) => mine.includes(t.id));
      case "tag_create": {
        const name = String(a.name);
        if (tags.some((t) => t.name.toLowerCase() === name.toLowerCase()))
          throw new Error(`a tag named ${name} already exists`);
        const row = { id: nextId++, name, color: a.color as TagColor };
        tags.push(row);
        calls.push(`create:${name}:${String(a.color)}`);
        return row;
      }
      case "artifact_tag_add":
        mine.push(Number(a.tagId));
        calls.push(`add:${String(a.tagId)}`);
        return undefined;
      case "artifact_tag_remove":
        mine = mine.filter((id) => id !== Number(a.tagId));
        calls.push(`remove:${String(a.tagId)}`);
        return undefined;
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls };
}

const rust: Row = { id: 1, name: "rust", color: "cyan" };
const web: Row = { id: 2, name: "web", color: "violet" };

async function panel() {
  return screen.findByRole("dialog", { name: "alpha details" });
}

describe("tags in the detail panel", () => {
  it("adds an existing tag and updates the detail, the card and the filter", async () => {
    const user = userEvent.setup();
    const { calls } = setup([rust, web]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    const list = await screen.findByRole("listbox");
    expect(within(list).getByText("rust")).toBeInTheDocument();
    await user.click(within(list).getByText("web"));
    await waitFor(() =>
      expect(
        within(p).getByRole("button", { name: "Remove tag web" }),
      ).toBeInTheDocument(),
    );
    expect(calls).toEqual(["add:2"]);
    expect(screen.getByRole("status")).toHaveTextContent("Added #web");
    // The card and the filter column (from facets) follow after the list reloads.
    await waitFor(() =>
      expect(screen.getAllByText("web").length).toBeGreaterThan(2),
    );
    // The tag just added is no longer offered.
    expect(within(screen.getByRole("listbox")).queryByText("web")).toBeNull();
  });

  it("creates a tag with the chosen color and puts it on the artifact", async () => {
    const user = userEvent.setup();
    const { calls } = setup([rust]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    await user.type(
      await screen.findByRole("combobox", { name: "Find or create a tag" }),
      "docs",
    );
    await user.click(screen.getByRole("radio", { name: "fuchsia" }));
    expect(screen.getByRole("radio", { name: "fuchsia" })).toBeChecked();
    expect(screen.getByText("fuchsia")).toBeVisible();
    await user.click(screen.getByRole("option", { name: /Create “docs”/ }));
    await waitFor(() =>
      expect(
        within(p).getByRole("button", { name: "Remove tag docs" }),
      ).toBeInTheDocument(),
    );
    expect(calls).toEqual(["create:docs:fuchsia", "add:100"]);
  });

  it("removes a tag from the artifact", async () => {
    const user = userEvent.setup();
    const { calls } = setup([rust, web], [1]);
    const p = await panel();
    await user.click(
      await within(p).findByRole("button", { name: "Remove tag rust" }),
    );
    await waitFor(() =>
      expect(
        within(p).queryByRole("button", { name: "Remove tag rust" }),
      ).toBeNull(),
    );
    expect(calls).toEqual(["remove:1"]);
    expect(screen.getByRole("status")).toHaveTextContent("Removed #rust");
  });

  it("never offers Create for a name that exists, ignoring what else is typed", async () => {
    const user = userEvent.setup();
    setup([rust]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    const box = await screen.findByRole("combobox", {
      name: "Find or create a tag",
    });
    await user.type(box, "rust");
    // Exact match: offered as the existing tag, never as "Create".
    expect(screen.queryByRole("option", { name: /Create/ })).toBeNull();
    await user.clear(box);
    await user.type(box, "ru");
    expect(screen.getByRole("option", { name: /Create “ru”/ })).toBeVisible();
  });

  it("reports a rejected create inline", async () => {
    const user = userEvent.setup();
    const created = { id: 7, name: "Docs", color: "cyan" as TagColor };
    // tags_list hides "Docs" so the menu offers Create, while tag_create rejects it.
    mockIPC((cmd) => {
      switch (cmd) {
        case "library_list":
          return page([item("alpha")]);
        case "library_detail":
          return detail;
        case "library_find":
          return null;
        case "tags_list":
          return [];
        case "artifact_tags":
          return [];
        case "tag_create":
          throw new Error(`a tag named ${created.name} already exists`);
        default:
          return undefined;
      }
    });
    render(
      <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    await user.type(await screen.findByRole("combobox"), "Docs{Enter}");
    expect(await within(p).findByRole("alert")).toHaveTextContent(
      "a tag named Docs already exists",
    );
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  it("moves with the arrow keys, picks with Enter and closes with Esc", async () => {
    const user = userEvent.setup();
    const { calls } = setup([rust, web]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    const box = await screen.findByRole("combobox");
    await waitFor(() => expect(box).toHaveFocus());
    const options = await screen.findAllByRole("option");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("option")[1]).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.keyboard("{ArrowUp}{ArrowUp}");
    // Wraps to the last option, web.
    expect(screen.getAllByRole("option")[1]).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await user.keyboard("{Enter}");
    await waitFor(() => expect(calls).toEqual(["add:2"]));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
    // The panel itself stays open: that Esc only closed the menu.
    expect(screen.getByRole("dialog", { name: "alpha details" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Add tag" })).toHaveFocus();
  });

  it("closes on an outside click, and says when there is nothing to add", async () => {
    const user = userEvent.setup();
    setup([rust], [1]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    expect(await screen.findByText("No other tags.")).toBeInTheDocument();
    await user.click(within(p).getByRole("heading", { name: "Tags" }));
    expect(screen.queryByRole("listbox")).toBeNull();
    await user.click(within(p).getByRole("button", { name: "Add tag" }));
    await user.type(screen.getByRole("combobox"), "rust");
    expect(
      screen.getByText("This artifact already has it."),
    ).toBeInTheDocument();
  });

  it("shows the empty-library hint", async () => {
    const user = userEvent.setup();
    setup([]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    expect(
      await screen.findByText("No tags yet. Type a name to create one."),
    ).toBeInTheDocument();
  });

  it("hover moves the highlight and a click picks", async () => {
    const user = userEvent.setup();
    setup([rust, web]);
    const p = await panel();
    await user.click(await within(p).findByRole("button", { name: "Add tag" }));
    const options = await screen.findAllByRole("option");
    await user.hover(options[1] as HTMLElement);
    expect(options[1]).toHaveAttribute("aria-selected", "true");
    await user.click(options[1] as HTMLElement);
  });
});

describe("tag IPC wrappers", () => {
  it("send numeric ids to the manage commands", async () => {
    const seen: unknown[] = [];
    mockIPC((cmd, args) => {
      seen.push([cmd, args]);
      return undefined;
    });
    await tagRename(3n, "x");
    await tagRecolor(3n, "sky");
    await tagDelete(3n);
    expect(seen).toEqual([
      ["tag_rename", { id: 3, name: "x" }],
      ["tag_recolor", { id: 3, color: "sky" }],
      ["tag_delete", { id: 3 }],
    ]);
  });
});

describe("tags load failure", () => {
  it("shows the error inline", async () => {
    mockIPC((cmd) => {
      switch (cmd) {
        case "library_list":
          return page([item("alpha")]);
        case "library_detail":
          return detail;
        case "tags_list":
          throw new Error("index is locked");
        case "artifact_tags":
          return [];
        default:
          return null;
      }
    });
    render(
      <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText(/index is locked/)).toBeInTheDocument();
  });
});
