import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ClaudeSettings } from "./ClaudeSettings";

function setup(stored: boolean | null, failSet = false) {
  const sets: boolean[] = [];
  mockIPC((cmd, args) => {
    switch (cmd) {
      case "claude_cli_status":
        return { found: true, path: "/bin/claude", version: "2" };
      case "import_layout_setting_get":
        return stored;
      case "import_layout_setting_set":
        if (failSet) throw new Error("read only");
        sets.push((args as { enabled: boolean }).enabled);
        return undefined;
      default:
        return null;
    }
  });
  render(<ClaudeSettings />);
  return sets;
}

const name = "Ask Claude about repo layouts at import";

describe("Settings: Ask Claude about repo layouts", () => {
  it("is off by default and says exactly what is sent and to whom", async () => {
    setup(false);
    const sw = await screen.findByRole("switch", { name });
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(sw).toHaveAccessibleDescription(/your own claude CLI/);
    expect(sw).toHaveAccessibleDescription(/file paths/);
    expect(sw).toHaveAccessibleDescription(/manifests/);
    expect(sw).toHaveAccessibleDescription(/first 40 lines/);
  });

  it("shows the stored value and persists a change", async () => {
    const u = userEvent.setup();
    const sets = setup(true);
    const sw = await screen.findByRole("switch", { name });
    await waitFor(() => expect(sw).toHaveAttribute("aria-checked", "true"));
    await u.click(sw);
    await waitFor(() => expect(sets).toEqual([false]));
    expect(sw).toHaveAttribute("aria-checked", "false");
  });

  it("puts the switch back and says why when saving fails", async () => {
    const u = userEvent.setup();
    setup(false, true);
    const sw = await screen.findByRole("switch", { name });
    await u.click(sw);
    expect(await screen.findByRole("alert")).toHaveTextContent("read only");
    expect(sw).toHaveAttribute("aria-checked", "false");
  });
});
