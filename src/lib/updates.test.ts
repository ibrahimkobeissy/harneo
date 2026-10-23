import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { autoCheckIfDue } from "./updates";

function setup(settings: Record<string, unknown>) {
  const calls: string[] = [];
  mockIPC((cmd, args) => {
    const { key, value } = (args ?? {}) as { key?: string; value?: unknown };
    if (cmd === "settings_get") return settings[String(key)] ?? null;
    if (cmd === "settings_set") {
      settings[String(key)] = value;
      return undefined;
    }
    if (cmd === "updates_check") {
      calls.push("check");
      return { sources: [] };
    }
    return undefined;
  });
  return calls;
}

const now = new Date("2026-10-23T12:00:00Z");

describe("automatic update check", () => {
  it("runs when there was no check yet, and records the time", async () => {
    const settings: Record<string, unknown> = {};
    const calls = setup(settings);
    expect(await autoCheckIfDue(now)).toBe(true);
    expect(calls).toEqual(["check"]);
    expect(typeof settings["updates.lastCheck"]).toBe("string");
  });

  it("runs when the last check is older than 24 hours", async () => {
    const calls = setup({ "updates.lastCheck": "2026-10-22T11:00:00Z" });
    expect(await autoCheckIfDue(now)).toBe(true);
    expect(calls).toEqual(["check"]);
  });

  it("waits when the last check is recent", async () => {
    const calls = setup({ "updates.lastCheck": "2026-10-23T01:00:00Z" });
    expect(await autoCheckIfDue(now)).toBe(false);
    expect(calls).toEqual([]);
  });

  it("does nothing when the setting is off", async () => {
    const calls = setup({ "updates.autoCheck": false });
    expect(await autoCheckIfDue(now)).toBe(false);
    expect(calls).toEqual([]);
  });
});
