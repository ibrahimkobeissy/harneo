import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DiagnosticsSettings } from "./DiagnosticsSettings";

function setup(
  status: { disabledByEnv: boolean; switchOn: boolean },
  failSave = false,
) {
  const sets: { key: string; value: unknown }[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "diagnostics_status") return status;
    if (cmd === "settings_set" && failSave) throw new Error("disk full");
    if (cmd === "plugin:opener|open_url") throw new Error("no browser");
    if (cmd === "settings_set") {
      const a = args as { key: string; value: unknown };
      sets.push({ key: a.key, value: a.value });
    }
    return undefined;
  });
  render(<DiagnosticsSettings />);
  return sets;
}

describe("Settings: Diagnostics", () => {
  it("is off and disabled, with instructions, when HARNEO_SENTRY_DSN is set empty", async () => {
    setup({ disabledByEnv: true, switchOn: false });
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(await screen.findByText(/set to an empty value/)).toBeVisible();
    expect(sw).toBeDisabled();
    expect(screen.getByRole("button", { name: "BUILDING.md" })).toBeVisible();
    expect(screen.getByText(/never attached/)).toBeVisible();
  });

  it("is off by default and saves when switched on", async () => {
    const sets = setup({ disabledByEnv: false, switchOn: false });
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    await waitFor(() => expect(sw).toBeEnabled());
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(screen.queryByText(/set to an empty value/)).toBeNull();
    await userEvent.click(sw);
    await waitFor(() =>
      expect(sets).toEqual([{ key: "diagnostics.sentryReports", value: true }]),
    );
    expect(sw).toHaveAttribute("aria-checked", "true");
  });

  it("undoes the switch and says so when saving fails", async () => {
    setup({ disabledByEnv: false, switchOn: false }, true);
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    await waitFor(() => expect(sw).toBeEnabled());
    await userEvent.click(sw);
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(sw).toHaveAttribute("aria-checked", "false");
  });

  it("reports a failure to open BUILDING.md", async () => {
    setup({ disabledByEnv: true, switchOn: false });
    await userEvent.click(
      await screen.findByRole("button", { name: "BUILDING.md" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not open",
    );
  });
});
