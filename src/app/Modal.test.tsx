import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Modal } from "./Modal";

describe("Modal", () => {
  it("traps Tab and Shift+Tab inside the dialog", async () => {
    const user = userEvent.setup();
    render(
      <>
        <button type="button">outside</button>
        <Modal label="Test dialog">
          <button type="button">first</button>
          <button type="button">last</button>
        </Modal>
      </>,
    );
    const dialog = screen.getByRole("dialog", { name: "Test dialog" });
    expect(dialog).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "first" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "last" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "first" })).toHaveFocus();
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "last" })).toHaveFocus();
  });

  it("restores focus on unmount and closes from the scrim", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    function Host({ open }: { open: boolean }) {
      return (
        <>
          <button type="button">opener</button>
          {open && (
            <Modal label="D" onClose={onClose}>
              <p>hi</p>
            </Modal>
          )}
        </>
      );
    }
    const { rerender, container } = render(<Host open={false} />);
    const opener = screen.getByRole("button", { name: "opener" });
    opener.focus();
    rerender(<Host open />);
    expect(screen.getByRole("dialog")).toHaveFocus();
    const scrim = container.querySelector('[aria-hidden="true"]');
    if (!(scrim instanceof HTMLElement)) throw new Error("no scrim");
    await user.click(scrim);
    expect(onClose).toHaveBeenCalledOnce();
    rerender(<Host open={false} />);
    expect(opener).toHaveFocus();
  });
});
