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
    expect(screen.getByText("Claude CLI")).toBeVisible();
    expect(screen.getByText("/home/me/.local/bin/claude")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Copy the Claude CLI path" }),
    ).toBeVisible();
    expect(screen.getByText("2.1.0")).toBeVisible();
  });

  it("says so when it is not found", async () => {
    setup({ found: false, path: null, version: null });
    expect(await screen.findByText("Not found")).toBeVisible();
    expect(
      screen.getByText(/won't be suggested at import; add them by hand/),
    ).toBeVisible();
  });

  it("offers install help and Check again, and disables Model and Detect, when the CLI is missing", async () => {
    const user = userEvent.setup();
    let calls = 0;
    mockIPC((cmd) => {
      if (cmd === "claude_cli_status") {
        calls += 1;
        return calls === 1
          ? { found: false, path: null, version: null }
          : { found: true, path: "/bin/claude", version: "2.1.0" };
      }
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return null;
    });
    render(<SettingsScreen />);
    expect(await screen.findByText("Not found")).toBeVisible();
    expect(screen.getByText(/Install Claude Code from/)).toBeVisible();
    expect(screen.getByRole("combobox", { name: "Model" })).toBeDisabled();
    expect(
      screen.getByRole("switch", { name: "Detect bundles and tags at import" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByText("Found")).toBeVisible();
    expect(screen.getByRole("combobox", { name: "Model" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Check again" })).toBeNull();
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

  it("defaults to Haiku with detection at import off", async () => {
    setup({ found: true, path: "/bin/claude", version: "2" });
    expect(await screen.findByRole("combobox", { name: "Model" })).toHaveValue(
      "haiku",
    );
    expect(
      screen.getByRole("switch", { name: "Detect bundles and tags at import" }),
    ).toHaveAttribute("aria-checked", "false");
    expect(
      screen.getByText(/Off by default\. When on, each import sends/),
    ).toBeVisible();
    expect(screen.getByText(/up to 6,000 characters each/)).toBeVisible();
  });
});

describe("update settings", () => {
  it("shows the toggle on by default and stores a change", async () => {
    const user = userEvent.setup();
    const stored: Record<string, unknown> = {};
    mockIPC((cmd, args) => {
      const { key, value } = (args ?? {}) as { key?: string; value?: unknown };
      if (cmd === "settings_get") return null;
      if (cmd === "settings_set") {
        stored[String(key)] = value;
        return undefined;
      }
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return undefined;
    });
    render(<SettingsScreen />);
    const box = await screen.findByRole("switch", {
      name: "Check for updates at start",
    });
    await waitFor(() => expect(box).toBeEnabled());
    expect(box).toHaveAttribute("aria-checked", "true");
    await user.click(box);
    await waitFor(() => expect(stored["updates.autoCheck"]).toBe(false));
    expect(box).toHaveAttribute("aria-checked", "false");
  });

  it("shows the toggle off when the setting is false", async () => {
    mockIPC((cmd) => {
      if (cmd === "settings_get") return false;
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return undefined;
    });
    render(<SettingsScreen />);
    const box = await screen.findByRole("switch", {
      name: "Check for updates at start",
    });
    await waitFor(() => expect(box).toHaveAttribute("aria-checked", "false"));
  });
});

describe("Settings: structure", () => {
  it("links every description to its control and shows On or Off", async () => {
    setup({ found: true, path: "/bin/claude", version: "2" });
    await screen.findByText("Found");
    const detect = screen.getByRole("switch", {
      name: "Detect bundles and tags at import",
    });
    expect(detect).toHaveAccessibleDescription(
      /Off by default.*6,000 characters/s,
    );
    expect(
      screen.getByRole("combobox", { name: "Model" }),
    ).toHaveAccessibleDescription(/Which model reads your artifacts/);
    expect(
      screen.getByRole("switch", { name: "Pause live rescan" }),
    ).toHaveAccessibleDescription(/it only reads/);
    expect(
      await screen.findByRole("switch", { name: "Check for updates at start" }),
    ).toHaveAccessibleDescription(/At start, at most once a day/);
    expect(screen.getAllByText("Off").length).toBeGreaterThan(0);
    expect(screen.getAllByText("On").length).toBeGreaterThan(0);
  });

  it("groups Updates and Live rescan under Background and shows the data folder once", async () => {
    setup({ found: true, path: "/bin/claude", version: "2" });
    await screen.findByText("Found");
    expect(screen.getByRole("heading", { name: "Background" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Updates" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Live rescan" })).toBeNull();
    expect(await screen.findAllByText("/data")).toHaveLength(1);
    expect(screen.getByText("0.1.0")).toBeVisible();
  });

  it("opens the install page through the opener when the CLI is missing", async () => {
    const user = userEvent.setup();
    const opened: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "claude_cli_status")
        return { found: false, path: null, version: null };
      if (cmd === "plugin:opener|open_url") {
        opened.push(args);
        return undefined;
      }
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return null;
    });
    render(<SettingsScreen />);
    await user.click(
      await screen.findByRole("button", { name: "code.claude.com" }),
    );
    await waitFor(() =>
      expect(opened).toEqual([
        { url: "https://code.claude.com", with: undefined },
      ]),
    );
  });

  it("says when the install page could not be opened", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "claude_cli_status")
        return { found: false, path: null, version: null };
      if (cmd === "plugin:opener|open_url") throw "not allowed";
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return null;
    });
    render(<SettingsScreen />);
    await user.click(
      await screen.findByRole("button", { name: "code.claude.com" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("not allowed");
  });
});

describe("Settings: confirm security findings", () => {
  it("is on by default and saves the change", async () => {
    const user = userEvent.setup();
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "claude_cli_status")
        return { found: true, path: "/bin/claude", version: "2" };
      if (cmd === "audit_confirm_setting_get") return true;
      if (cmd === "app_info") return { version: "0.1.0", dataDir: "/data" };
      return null;
    });
    render(<SettingsScreen />);
    const sw = await screen.findByRole("switch", {
      name: "Confirm security findings with Claude",
    });
    expect(sw).toBeChecked();
    expect(sw).toHaveAccessibleDescription(/rule, file and line/);
    expect(sw).toHaveAccessibleDescription(/name and description/);
    expect(sw).toHaveAccessibleDescription(/your own claude CLI/);
    await user.click(sw);
    expect(sw).not.toBeChecked();
    await waitFor(() =>
      expect(calls).toContainEqual({
        cmd: "audit_confirm_setting_set",
        args: { enabled: false },
      }),
    );
  });
});
