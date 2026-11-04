import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DiagnosticsSettings } from "./DiagnosticsSettings";

function setup(
  status: { dsnMissing: boolean; switchOn: boolean },
  failSave = false,
) {
  const sets: { key: string; value: unknown }[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "diagnostics_status") return status;
    if (cmd === "settings_set" && failSave) throw new Error("disk full");
    if (cmd === "open_link") throw new Error("no browser");
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
  it("is off and disabled, with instructions, when HARNEO_SENTRY_DSN is not set", async () => {
    setup({ dsnMissing: true, switchOn: false });
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(await screen.findByText(/Needs a Sentry project/)).toBeVisible();
    expect(sw).toBeDisabled();
    expect(screen.getByRole("button", { name: "BUILDING.md" })).toBeVisible();
    expect(screen.getByText(/never attached/)).toBeVisible();
  });

  it("shows the switch on when the DSN is set and no choice was made", async () => {
    setup({ dsnMissing: false, switchOn: true });
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    await waitFor(() => expect(sw).toHaveAttribute("aria-checked", "true"));
    expect(sw).toBeEnabled();
    expect(screen.getByText(/On by default once/)).toBeVisible();
  });

  it("saves when switched on after being turned off", async () => {
    const sets = setup({ dsnMissing: false, switchOn: false });
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    await waitFor(() => expect(sw).toBeEnabled());
    expect(sw).toHaveAttribute("aria-checked", "false");
    expect(screen.queryByText(/Needs a Sentry project/)).toBeNull();
    await userEvent.click(sw);
    await waitFor(() =>
      expect(sets).toEqual([{ key: "diagnostics.sentryReports", value: true }]),
    );
    expect(sw).toHaveAttribute("aria-checked", "true");
  });

  it("undoes the switch and says so when saving fails", async () => {
    setup({ dsnMissing: false, switchOn: false }, true);
    const sw = screen.getByRole("switch", {
      name: "Send error reports to Sentry",
    });
    await waitFor(() => expect(sw).toBeEnabled());
    await userEvent.click(sw);
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(sw).toHaveAttribute("aria-checked", "false");
  });

  it("reports a failure to open BUILDING.md", async () => {
    setup({ dsnMissing: true, switchOn: false });
    await userEvent.click(
      await screen.findByRole("button", { name: "BUILDING.md" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not open",
    );
  });
});
