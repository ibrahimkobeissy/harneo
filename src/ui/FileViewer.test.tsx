import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { FileViewer, isMarkdown, languageOf } from ".";

describe("languageOf", () => {
  it("maps extensions to Prism languages and falls back to plain text", () => {
    expect(languageOf("scripts/a.py")).toBe("python");
    expect(languageOf("x.JS")).toBe("javascript");
    expect(languageOf("Makefile")).toBe("text");
    expect(languageOf("run.sh")).toBe("text");
    expect(isMarkdown("a/SKILL.md")).toBe(true);
    expect(isMarkdown("a.py")).toBe(false);
  });
});

describe("FileViewer", () => {
  it("renders Markdown, with a Source toggle to the raw text", async () => {
    const user = userEvent.setup();
    render(
      <FileViewer
        path="SKILL.md"
        text={"# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |"}
      />,
    );
    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
    expect(screen.getByRole("table")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Source" }));
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Contents of SKILL.md")).toHaveTextContent(
      "# Title",
    );
    await user.click(screen.getByRole("button", { name: "Source" }));
    expect(screen.getByRole("heading", { name: "Title" })).toBeInTheDocument();
  });

  it("shows raw HTML as text, loads no remote image and does not follow links", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <FileViewer
        path="a.md"
        text={
          "<script>alert(1)</script>\n\n![logo](https://example.com/x.png)\n\n[docs](https://example.com)"
        }
      />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(
      screen.getByText(/<script>alert\(1\)<\/script>/),
    ).toBeInTheDocument();
    expect(
      screen.getByText("[image: logo (https://example.com/x.png)]"),
    ).toBeInTheDocument();
    const link = screen.getByRole("link", { name: "docs" });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    link.dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    await user.click(link);
  });

  it("shows code read-only with line numbers", () => {
    render(<FileViewer path="scripts/a.py" text={"x = 1\nprint(x)\n"} />);
    expect(
      screen.getByText("1", { selector: "span[aria-hidden]" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("2", { selector: "span[aria-hidden]" }),
    ).toBeInTheDocument();
    expect(screen.getByText("print")).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Source" })).toBeNull();
  });

  it("shows the explanation when there is no text", () => {
    render(<FileViewer path="a.bin" text={null} empty={<p>No preview</p>} />);
    expect(screen.getByText("No preview")).toBeInTheDocument();
  });

  it("shows an image without alt text or URL as a plain marker", () => {
    render(<FileViewer path="a.md" text={"![](<>)"} />);
    expect(screen.getByText("[image]")).toBeInTheDocument();
  });
});
