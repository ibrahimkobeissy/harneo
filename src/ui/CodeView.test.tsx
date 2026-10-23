import { render, screen } from "@testing-library/react";
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
    expect(screen.getByRole("button", { name: /z\.md/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByText("replace")).toBeInTheDocument();
    expect(screen.getByText("kept")).toBeInTheDocument();
    expect(screen.getByText("delete")).toBeInTheDocument();
  });
});
