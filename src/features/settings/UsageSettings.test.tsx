import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
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

  it("saves a valid window and ignores an invalid one", async () => {
    const s = setup();
    const input = await screen.findByLabelText("Unused after N weeks");
    await userEvent.clear(input);
    expect(screen.getByRole("alert")).toHaveTextContent("1 to 52");
    await userEvent.type(input, "99");
    expect(s.sets).not.toContainEqual({
      key: "usage.unusedWeeks",
      value: 99,
    });
    await userEvent.clear(input);
    await userEvent.type(input, "6");
    await waitFor(() =>
      expect(s.sets).toContainEqual({ key: "usage.unusedWeeks", value: 6 }),
    );
  });

  it("shows when saving fails", async () => {
    setup({}, true);
    const input = await screen.findByLabelText("Unused after N weeks");
    await userEvent.clear(input);
    await userEvent.type(input, "5");
    expect((await screen.findAllByText(/read only/)).length).toBeGreaterThan(0);
  });
});
