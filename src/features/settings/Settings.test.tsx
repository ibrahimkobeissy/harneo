import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { CliStatus } from "../../bindings/CliStatus";
import { SettingsScreen } from ".";

function setup(cli: CliStatus, stored: Record<string, unknown> = {}) {
  const sets: { key: string; value: unknown }[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as { key?: string; value?: unknown };
    switch (cmd) {
      case "claude_cli_status":
        return cli;
      case "settings_get":
        return stored[a.key ?? ""] ?? null;
      case "settings_set":
        sets.push({ key: a.key ?? "", value: a.value });
        return undefined;
      case "app_info":
        return { version: "0.1.0", dataDir: "/data" };
      default:
        return undefined;
    }
  });
  render(<SettingsScreen />);
  return sets;
}

describe("Settings: Claude CLI", () => {
  it("shows where the CLI was found and its version", async () => {
    setup({
      found: true,
      path: "/home/me/.local/bin/claude",
      version: "2.1.0",
    });
    expect(await screen.findByText("Found")).toBeVisible();
    expect(screen.getByText("/home/me/.local/bin/claude")).toBeVisible();
    expect(screen.getByText("2.1.0")).toBeVisible();
  });

  it("says so when it is not found", async () => {
    setup({ found: false, path: null, version: null });
    expect(await screen.findByText("Not found")).toBeVisible();
    expect(
      screen.getByText(/won't be suggested at import; add them by hand/),
    ).toBeVisible();
  });

  it("loads the stored model and toggle, and saves changes through settings_set", async () => {
    const user = userEvent.setup();
    const sets = setup(
      { found: true, path: "/bin/claude", version: "2" },
      { "bundles.model": "sonnet", "bundles.atImport": false },
    );
    const model = await screen.findByRole("combobox", { name: "Model" });
    await waitFor(() => expect(model).toHaveValue("sonnet"));
    const toggle = screen.getByRole("switch", {
      name: "Detect bundles and tags at import",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");

    await user.selectOptions(model, "opus");
    await user.click(toggle);
    await waitFor(() =>
      expect(sets).toEqual([
        { key: "bundles.model", value: "opus" },
        { key: "bundles.atImport", value: true },
      ]),
    );
    expect(toggle).toHaveAttribute("aria-checked", "true");
  });

  it("defaults to Haiku with detection on", async () => {
    setup({ found: true, path: "/bin/claude", version: "2" });
    expect(await screen.findByRole("combobox", { name: "Model" })).toHaveValue(
      "haiku",
    );
    expect(
      screen.getByRole("switch", { name: "Detect bundles and tags at import" }),
    ).toHaveAttribute("aria-checked", "true");
  });
});
