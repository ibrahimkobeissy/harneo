import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DangerZone, reloadToLibrary } from "./DangerZone";

function setup(fail?: string) {
  const calls: unknown[] = [];
  const onDone = vi.fn();
  mockIPC((cmd, args) => {
    if (cmd === "reset_harneo") {
      calls.push(args);
      if (fail) throw new Error(fail);
    }
    return undefined;
  });
  render(<DangerZone onDone={onDone} />);
  return { calls, onDone };
}

describe("Settings: Danger zone", () => {
  it("shows the group with a Reset Harneo… button", () => {
    setup();
    expect(screen.getByRole("heading", { name: "Danger zone" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Reset Harneo…" })).toBeVisible();
  });

  it("keeps the confirm button disabled until reset is typed", async () => {
    const user = userEvent.setup();
    const { calls, onDone } = setup();
    await user.click(screen.getByRole("button", { name: "Reset Harneo…" }));
    const confirm = screen.getByRole("button", { name: "Reset Harneo" });
    expect(confirm).toBeDisabled();
    const input = screen.getByLabelText(/Type reset to confirm/);
    await user.type(input, "rese");
    expect(confirm).toBeDisabled();
    await user.type(input, "t");
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(calls).toEqual([{ deleteBackups: false }]);
  });

  it("warns and sends the choice when Also delete backups is ticked", async () => {
    const user = userEvent.setup();
    const { calls } = setup();
    await user.click(screen.getByRole("button", { name: "Reset Harneo…" }));
    expect(screen.queryByRole("note")).toBeNull();
    await user.click(screen.getByLabelText("Also delete backups"));
    expect(screen.getByRole("note")).toBeVisible();
    await user.type(screen.getByLabelText(/Type reset/), "reset");
    await user.click(screen.getByRole("button", { name: "Reset Harneo" }));
    await waitFor(() => expect(calls).toEqual([{ deleteBackups: true }]));
  });

  it("shows a failure and stays open", async () => {
    const user = userEvent.setup();
    const { onDone } = setup("Deleted: library. Could not delete: x.");
    await user.click(screen.getByRole("button", { name: "Reset Harneo…" }));
    await user.type(screen.getByLabelText(/Type reset/), "reset");
    await user.click(screen.getByRole("button", { name: "Reset Harneo" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not delete",
    );
    expect(onDone).not.toHaveBeenCalled();
  });

  it("reloads onto the Library", () => {
    const reload = vi.fn();
    const original = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { hash: "", reload },
    });
    reloadToLibrary();
    expect(window.location.hash).toBe("#/library");
    expect(reload).toHaveBeenCalled();
    Object.defineProperty(window, "location", {
      configurable: true,
      value: original,
    });
  });
});
