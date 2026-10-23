import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { UpdatesPanel } from "./UpdatesPanel";

const name = "Check for updates at start";

describe("UpdatesPanel", () => {
  it("falls back to on when the setting cannot be read", async () => {
    mockIPC((cmd) => {
      if (cmd === "settings_get") throw "unreadable";
      return undefined;
    });
    render(<UpdatesPanel />);
    const box = screen.getByRole("switch", { name });
    await waitFor(() => expect(box).toBeEnabled());
    expect(box).toHaveAttribute("aria-checked", "true");
  });

  it("reverts the toggle and shows the error when saving fails", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "settings_get") return null;
      if (cmd === "settings_set") throw "read-only";
      return undefined;
    });
    render(<UpdatesPanel />);
    const box = screen.getByRole("switch", { name });
    await waitFor(() => expect(box).toBeEnabled());
    await user.click(box);
    expect(await screen.findByRole("alert")).toHaveTextContent("read-only");
    expect(box).toHaveAttribute("aria-checked", "true");
  });
});
