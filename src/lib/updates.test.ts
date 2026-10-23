import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import {
  CHECKED_EVENT,
  autoCheckIfDue,
  readAutoCheck,
  readLastCheck,
  runCheck,
} from "./updates";

function mock(settings: Record<string, unknown>) {
  const calls: string[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as { value?: unknown } & Record<string, unknown>;
    const name = String(a["key"]);
    if (cmd === "settings_get") return settings[name] ?? null;
    if (cmd === "settings_set") {
      settings[name] = a.value;
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

describe("update checks", () => {
  it("reads the last check, ignoring values that are not dates", async () => {
    mock({ "updates.lastCheck": "garbage" });
    expect(await readLastCheck()).toBeNull();
    mock({ "updates.lastCheck": "2026-10-01T00:00:00.000Z" });
    expect((await readLastCheck())?.toISOString()).toBe(
      "2026-10-01T00:00:00.000Z",
    );
    mock({});
    expect(await readLastCheck()).toBeNull();
  });

  it("is on unless the setting is false", async () => {
    mock({});
    expect(await readAutoCheck()).toBe(true);
    mock({ "updates.autoCheck": false });
    expect(await readAutoCheck()).toBe(false);
  });

  it("runs a check, stores the time and fires the event", async () => {
    const settings: Record<string, unknown> = {};
    mock(settings);
    let seen = 0;
    const on = () => seen++;
    window.addEventListener(CHECKED_EVENT, on);
    await runCheck();
    window.removeEventListener(CHECKED_EVENT, on);
    expect(seen).toBe(1);
    expect(typeof settings["updates.lastCheck"]).toBe("string");
  });

  it("skips when switched off or checked within a day, and runs otherwise", async () => {
    const now = new Date("2026-10-23T12:00:00.000Z");
    mock({ "updates.autoCheck": false });
    expect(await autoCheckIfDue(now)).toBe(false);
    let calls = mock({ "updates.lastCheck": "2026-10-23T00:00:00.000Z" });
    expect(await autoCheckIfDue(now)).toBe(false);
    expect(calls).toEqual([]);
    calls = mock({ "updates.lastCheck": "2026-10-20T00:00:00.000Z" });
    expect(await autoCheckIfDue(now)).toBe(true);
    expect(calls).toEqual(["check"]);
    mock({});
    expect(await autoCheckIfDue(now)).toBe(true);
  });
});
