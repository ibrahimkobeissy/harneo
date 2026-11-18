import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PartNeedsNotice, type PartNeedRow } from "./PartNeedsNotice";

const row = (over: Partial<PartNeedRow> = {}): PartNeedRow => ({
  id: "r",
  part: "build",
  kind: "command",
  needs: "incremental-implementation",
  canKeep: true,
  onKeep: () => {},
  onDropBoth: () => {},
  ...over,
});

describe("PartNeedsNotice", () => {
  it("shows nothing without rows", () => {
    const { container } = render(<PartNeedsNotice rows={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("calls a command with its slash and a skill by its name", () => {
    render(
      <PartNeedsNotice
        rows={[
          row(),
          row({ id: "s", part: "planning", kind: "skill", needs: "tdd" }),
        ]}
      />,
    );
    const list = screen.getByRole("list", {
      name: "Parts that need a dropped part",
    });
    expect(list).toHaveTextContent("/build needs incremental-implementation");
    expect(list).toHaveTextContent("planning needs tdd");
    expect(
      screen.getByText("2 kept parts need a part that is not kept."),
    ).toBeVisible();
  });

  it("offers each way out and nothing is called before a click", async () => {
    const onKeep = vi.fn();
    const onDropBoth = vi.fn();
    render(<PartNeedsNotice rows={[row({ onKeep, onDropBoth })]} />);
    expect(onKeep).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Keep incremental-implementation" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Drop both" }));
    expect(onKeep).toHaveBeenCalledOnce();
    expect(onDropBoth).toHaveBeenCalledOnce();
  });

  it("says a needed part the library lacks cannot be kept from here", () => {
    render(<PartNeedsNotice rows={[row({ canKeep: false })]} />);
    expect(screen.getByText(/it is not in your library/)).toBeVisible();
    expect(screen.queryByRole("button", { name: /^Keep/ })).toBeNull();
    expect(screen.getByRole("button", { name: "Drop both" })).toBeVisible();
  });
});
