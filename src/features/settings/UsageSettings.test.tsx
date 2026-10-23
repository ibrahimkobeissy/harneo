import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UsageIndexContext } from "../../app/usageIndex";
import { UsageSettings } from "./UsageSettings";

function setup(
  stored: Record<string, unknown> = {},
  failSet = false,
  index: Partial<
    React.ComponentProps<typeof UsageIndexContext.Provider>["value"]
  > = {},
) {
  const sets: { key: string; value: unknown }[] = [];
  let refreshed = 0;
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as { key?: string; value?: unknown };
    if (cmd === "settings_get") return stored[a.key ?? ""] ?? null;
    if (cmd === "settings_set") {
      if (failSet) throw new Error("read only");
      sets.push({ key: a.key ?? "", value: a.value });
      return undefined;
    }
    if (cmd === "app_info")
      return { version: "0.1.0", dataDir: "/data/harneo" };
    return undefined;
  });
  render(
    <UsageIndexContext.Provider
      value={{
        indexed: true,
        progress: null,
        error: null,
        runs: 0,
        refresh: () => {
          refreshed += 1;
        },
        ...index,
      }}
    >
      <UsageSettings />
    </UsageIndexContext.Provider>,
  );
  return { sets, refreshed: () => refreshed };
}

describe("Settings: usage rows", () => {
  it("shows the data folder, the logs path and the last index time", async () => {
    setup({
      "usage.lastIndexedAt": "2026-10-20T10:00:00Z",
      "usage.unusedWeeks": 12,
    });
    expect(await screen.findByText("/data/harneo")).toBeVisible();
    expect(screen.getByText("~/.claude/projects/")).toBeVisible();
    await waitFor(() =>
      expect(screen.getByTestId("last-indexed")).not.toHaveTextContent("never"),
    );
    expect(await screen.findByDisplayValue("12")).toBeVisible();
  });

  it("says never when nothing was indexed and re-indexes on click", async () => {
    const s = setup();
    expect(screen.getByTestId("last-indexed")).toHaveTextContent("never");
    await userEvent.click(screen.getByRole("button", { name: "Re-index" }));
    expect(s.refreshed()).toBe(1);
  });

  it("shows progress and disables Re-index while running", () => {
    setup({}, false, { progress: { filesDone: 2, filesTotal: 9, newUses: 0 } });
    expect(screen.getByRole("button", { name: "Indexing…" })).toBeDisabled();
    expect(screen.getByText("Reading logs: 2 of 9 files")).toBeVisible();
  });

  it("shows progress before the file count is known and an index error", () => {
    setup({}, false, {
      progress: { filesDone: 0, filesTotal: 0, newUses: 0 },
      error: "no logs",
    });
    expect(screen.getByText("Reading logs…")).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("no logs");
  });

  it("saves once on blur, not per keystroke, and confirms with Saved", async () => {
    const s = setup();
    const input = await screen.findByLabelText("Unused after N weeks");
    await userEvent.clear(input);
    expect(screen.getByRole("alert")).toHaveTextContent("1 to 52");
    await userEvent.type(input, "12");
    expect(s.sets).toEqual([]);
    expect(screen.queryByText("Saved")).toBeNull();
    await userEvent.tab();
    await waitFor(() =>
      expect(s.sets).toEqual([{ key: "usage.unusedWeeks", value: 12 }]),
    );
    expect(await screen.findByText("Saved")).toBeVisible();
    await userEvent.type(input, "3");
    expect(screen.queryByText("Saved")).toBeNull();
  });

  it("saves on Enter, skips an unchanged value and ignores an invalid one", async () => {
    const s = setup({ "usage.unusedWeeks": 8 });
    const input = await screen.findByDisplayValue("8");
    await userEvent.click(input);
    await userEvent.tab();
    expect(s.sets).toEqual([]);
    await userEvent.clear(input);
    await userEvent.type(input, "99{Enter}");
    await userEvent.tab();
    expect(s.sets).toEqual([]);
    await userEvent.clear(input);
    await userEvent.type(input, "6{Enter}");
    await waitFor(() =>
      expect(s.sets).toEqual([{ key: "usage.unusedWeeks", value: 6 }]),
    );
  });

  it("links the description to the input", async () => {
    setup();
    expect(
      await screen.findByLabelText("Unused after N weeks"),
    ).toHaveAccessibleDescription(/no use for this long/);
  });

  it("copies the data folder and logs paths and says Copied", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByText("/data/harneo");
    await user.click(
      screen.getByRole("button", { name: "Copy the data folder path" }),
    );
    expect(await screen.findByText("Copied")).toBeVisible();
    expect(await navigator.clipboard.readText()).toBe("/data/harneo");
    await user.click(
      screen.getByRole("button", { name: "Copy the session logs path" }),
    );
    expect(await navigator.clipboard.readText()).toBe("~/.claude/projects/");
  });

  it("says so when the clipboard refuses", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByText("/data/harneo");
    vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(
      new Error("denied"),
    );
    await user.click(
      screen.getByRole("button", { name: "Copy the data folder path" }),
    );
    expect(await screen.findByText("Not copied")).toBeVisible();
  });

  it("shows when saving fails", async () => {
    setup({}, true);
    const input = await screen.findByLabelText("Unused after N weeks");
    await userEvent.clear(input);
    await userEvent.type(input, "5");
    await userEvent.tab();
    expect((await screen.findAllByText(/read only/)).length).toBeGreaterThan(0);
  });
});
