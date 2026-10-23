import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { LiveRescan } from "./LiveRescan";

afterEach(() => clearMocks());

function setup(status: unknown, failPause = false) {
  const pauses: boolean[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "watch_status") return status;
    if (cmd === "watch_pause") {
      if (failPause) throw new Error("disk full");
      pauses.push((args as { paused: boolean }).paused);
    }
    return undefined;
  });
  render(<LiveRescan />);
  return pauses;
}

describe("Settings: Live rescan", () => {
  it("shows how many projects are watched and pauses on the switch", async () => {
    const user = userEvent.setup();
    const pauses = setup({ watching: 2, paused: false });
    expect(await screen.findByText(/Watching 2 projects/)).toBeVisible();
    const toggle = screen.getByRole("switch", { name: "Pause live rescan" });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-checked", "true");
    expect(pauses).toEqual([true]);
    expect(screen.getByText(/\(paused\)/)).toBeVisible();
  });

  it("uses the singular and shows a stored pause", async () => {
    setup({ watching: 1, paused: true });
    expect(await screen.findByText(/Watching 1 project\b/)).toBeVisible();
    expect(screen.getByRole("switch")).toHaveAttribute("aria-checked", "true");
  });

  it("puts the switch back and says why when the pause is not saved", async () => {
    const user = userEvent.setup();
    setup({ watching: 0, paused: false }, true);
    const toggle = await screen.findByRole("switch");
    await waitFor(() => expect(toggle).toBeEnabled());
    await user.click(toggle);
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  it("reports a status that cannot be read, and ignores the switch", async () => {
    mockIPC((cmd) => {
      if (cmd === "watch_status") throw new Error("no watcher");
      return undefined;
    });
    render(<LiveRescan />);
    expect(await screen.findByRole("alert")).toHaveTextContent("no watcher");
    expect(screen.getByRole("switch")).toBeDisabled();
  });
});
