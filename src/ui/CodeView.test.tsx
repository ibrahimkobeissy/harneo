import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Hunk } from "../bindings/Hunk";
import { CodeView, decodeText, FileTree, markOf } from ".";

describe("decodeText", () => {
  it("decodes UTF-8 and rejects binary or bad base64", () => {
    // "héllo" as UTF-8 bytes.
    expect(decodeText("aMOpbGxv")).toBe("héllo");
    expect(decodeText(btoa(String.fromCharCode(0xff, 0xfe, 0x00)))).toBeNull();
    expect(decodeText("***")).toBeNull();
  });
});

describe("CodeView", () => {
  it("numbers the lines of UTF-8 content", () => {
    render(<CodeView path="a.md" bytes={btoa("one\ntwo\n")} />);
    expect(screen.getByText("2 lines")).toBeInTheDocument();
    expect(screen.getByText("two")).toBeInTheDocument();
  });

  it("can leave out the path bar", () => {
    const { container } = render(
      <CodeView path="a.md" bytes={btoa("one\n")} hideHead />,
    );
    expect(screen.queryByText("1 line")).toBeNull();
    expect(container.textContent).not.toContain("a.md");
    expect(
      screen.getByRole("region", { name: "Content of a.md" }),
    ).toBeInTheDocument();
  });

  it("says so for a binary file and for no content", () => {
    const { rerender } = render(
      <CodeView path="x.bin" bytes={btoa(String.fromCharCode(0xff, 0xfe))} />,
    );
    expect(screen.getByRole("note")).toHaveTextContent("Binary file");
    rerender(<CodeView path="x.bin" bytes={null} />);
    expect(screen.getByText(/no content/)).toBeInTheDocument();
  });

  it("renders a diff with numbers and marks", () => {
    const hunks: Hunk[] = [
      {
        oldStart: 3,
        newStart: 3,
        lines: [
          { kind: "context", text: "keep" },
          { kind: "removed", text: "old" },
          { kind: "added", text: "new" },
        ],
      },
      { oldStart: 20, newStart: 20, lines: [{ kind: "added", text: "late" }] },
    ];
    render(<CodeView path="a.md" bytes={null} hunks={hunks} />);
    expect(screen.getByText("+2 −1")).toBeInTheDocument();
    expect(screen.getByText("old")).toBeInTheDocument();
    expect(screen.getByText("late")).toBeInTheDocument();
  });
});

describe("FileTree", () => {
  it("maps plan actions to marks", () => {
    expect(
      (["new", "replace", "delete", "keep", "skip"] as const).map(markOf),
    ).toEqual(["new", "replace", "delete", "kept", undefined]);
  });

  it("nests folders and marks the selected file", () => {
    render(
      <FileTree
        files={[
          { path: "a/b/c.md", mark: "replace" },
          { path: "z.md", mark: "kept" },
          { path: "a/d.md", mark: "delete" },
          { path: "///" },
        ]}
        selected="z.md"
        onSelect={() => undefined}
      />,
    );
    expect(screen.getByText("b/")).toBeInTheDocument();
    expect(screen.getByRole("treeitem", { name: /z\.md/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByText("replace")).toBeInTheDocument();
    expect(screen.getByText("kept")).toBeInTheDocument();
    expect(screen.getByText("delete")).toBeInTheDocument();
  });

  describe("as a tree", () => {
    const files = [{ path: "a/b/c.md" }, { path: "a/d.md" }, { path: "z.md" }];
    function tree(selected: string | null = null) {
      const picked: string[] = [];
      render(
        <FileTree
          files={files}
          selected={selected}
          onSelect={(p) => picked.push(p)}
        />,
      );
      return picked;
    }

    it("has tree semantics with expandable folders and one tab stop", () => {
      tree("z.md");
      expect(screen.getByRole("tree", { name: "Files" })).toBeInTheDocument();
      expect(screen.getAllByRole("group")).toHaveLength(2);
      const a = screen.getByRole("treeitem", { name: "a/" });
      expect(a).toHaveAttribute("aria-expanded", "true");
      const stops = screen
        .getAllByRole("treeitem")
        .filter((n) => n.getAttribute("tabindex") === "0");
      expect(stops).toHaveLength(1);
      expect(stops[0]).toHaveTextContent("z.md");
    });

    it("starts the tab stop on the first row when nothing is selected", () => {
      tree();
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveAttribute(
        "tabindex",
        "0",
      );
    });

    it("moves with the arrow keys, Home and End", async () => {
      const u = userEvent.setup();
      tree();
      screen.getByRole("treeitem", { name: "a/" }).focus();
      await u.keyboard("{ArrowDown}");
      expect(screen.getByRole("treeitem", { name: "b/" })).toHaveFocus();
      await u.keyboard("{ArrowDown}{ArrowDown}");
      expect(screen.getByRole("treeitem", { name: "d.md" })).toHaveFocus();
      await u.keyboard("{End}");
      expect(screen.getByRole("treeitem", { name: "z.md" })).toHaveFocus();
      await u.keyboard("{ArrowDown}");
      expect(screen.getByRole("treeitem", { name: "z.md" })).toHaveFocus();
      await u.keyboard("{ArrowUp}");
      expect(screen.getByRole("treeitem", { name: "d.md" })).toHaveFocus();
      await u.keyboard("{Home}");
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveFocus();
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveAttribute(
        "tabindex",
        "0",
      );
    });

    it("collapses and expands folders with Left and Right", async () => {
      const u = userEvent.setup();
      tree();
      screen.getByRole("treeitem", { name: "a/" }).focus();
      await u.keyboard("{ArrowRight}");
      expect(screen.getByRole("treeitem", { name: "b/" })).toHaveFocus();
      await u.keyboard("{ArrowRight}{ArrowRight}");
      expect(screen.getByRole("treeitem", { name: "c.md" })).toHaveFocus();
      await u.keyboard("{ArrowLeft}");
      expect(screen.getByRole("treeitem", { name: "b/" })).toHaveFocus();
      await u.keyboard("{ArrowLeft}");
      expect(screen.getByRole("treeitem", { name: "b/" })).toHaveAttribute(
        "aria-expanded",
        "false",
      );
      expect(screen.queryByRole("treeitem", { name: "c.md" })).toBeNull();
      await u.keyboard("{ArrowRight}");
      expect(screen.getByRole("treeitem", { name: "c.md" })).toBeVisible();
      await u.keyboard("{ArrowLeft}{ArrowLeft}");
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveFocus();
      await u.keyboard("{ArrowLeft}");
      expect(screen.queryByRole("treeitem", { name: "d.md" })).toBeNull();
      // A closed folder with nothing to step into, and Left at the top, do nothing.
      await u.keyboard("{ArrowLeft}");
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveFocus();
    });

    it("selects a file with Enter or Space or a click; folders toggle", async () => {
      const u = userEvent.setup();
      const picked = tree();
      screen.getByRole("treeitem", { name: "z.md" }).focus();
      await u.keyboard("{Enter}");
      await u.keyboard(" ");
      await u.click(screen.getByRole("treeitem", { name: "d.md" }));
      expect(picked).toEqual(["z.md", "z.md", "a/d.md"]);
      screen.getByRole("treeitem", { name: "a/" }).focus();
      await u.keyboard("{Enter}");
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveAttribute(
        "aria-expanded",
        "false",
      );
      await u.click(screen.getByRole("treeitem", { name: "a/" }));
      expect(screen.getByRole("treeitem", { name: "a/" })).toHaveAttribute(
        "aria-expanded",
        "true",
      );
    });

    it("leaves other keys alone", async () => {
      const u = userEvent.setup();
      const picked = tree();
      screen.getByRole("treeitem", { name: "z.md" }).focus();
      await u.keyboard("x");
      expect(picked).toEqual([]);
    });
  });
});
