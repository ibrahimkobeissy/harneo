import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { FileView } from "../../bindings/FileView";
import { FileDialog } from "./FileDialog";

function setup(view: FileView | Error, path = "SKILL.md") {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "library_read_file") {
      if (view instanceof Error) throw view;
      return view;
    }
    return undefined;
  });
  const onClose = vi.fn();
  render(<FileDialog id="a1" path={path} onClose={onClose} />);
  return { calls, onClose };
}

describe("file dialog", () => {
  it("shows a Markdown file rendered, titled with its path", async () => {
    const { calls } = setup({ kind: "text", text: "# Hello" });
    expect(
      await screen.findByRole("heading", { name: "Hello", level: 1 }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("dialog", { name: "SKILL.md" }),
    ).toBeInTheDocument();
    expect(calls[0]).toEqual({
      cmd: "library_read_file",
      args: { id: "a1", path: "SKILL.md" },
    });
  });

  it("shows a script as highlighted code with line numbers", async () => {
    setup({ kind: "text", text: "print(1)\n" }, "scripts/a.py");
    expect(await screen.findByText("print")).toBeInTheDocument();
    expect(
      screen.getByText("1", { selector: "span[aria-hidden]" }),
    ).toBeInTheDocument();
  });

  it("shows no preview for a binary or too large file, and Open in editor opens it", async () => {
    const user = userEvent.setup();
    const { calls } = setup({ kind: "tooLarge", size: 2_000_000 }, "big.txt");
    expect(await screen.findByText(/over 1 MB/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Open in editor/ }));
    expect(calls.at(-1)).toEqual({
      cmd: "library_open_file",
      args: { id: "a1", path: "big.txt" },
    });
  });

  it("explains a binary file", async () => {
    setup({ kind: "binary" }, "x.png");
    expect(await screen.findByText(/not text/)).toBeInTheDocument();
  });

  it("closes on Esc and on the close button, and reports a refusal", async () => {
    const user = userEvent.setup();
    const { onClose } = setup({ kind: "text", text: "x" }, "a.txt");
    await screen.findByText("x");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("shows a refusal from the backend", async () => {
    setup(new Error("unsafe path in the source: ../x"), "../x");
    expect(await screen.findByRole("alert")).toHaveTextContent("unsafe path");
  });
});
