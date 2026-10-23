import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import * as ipc from "./ipc";

afterEach(() => clearMocks());

describe("live rescan wrappers", () => {
  it("onChanged hands the event payload to the handler until stopped", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: unknown[] = [];
    const stop = await ipc.onChanged((scope) => seen.push(scope));
    await emit("harneo://changed", { scope: "library" });
    await emit("harneo://changed", { scope: "project", id: 3 });
    expect(seen).toEqual([{ scope: "library" }, { scope: "project", id: 3 }]);
    stop();
    await emit("harneo://changed", { scope: "library" });
    expect(seen).toHaveLength(2);
  });

  it("watchStatus and watchPause send their commands", async () => {
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return { watching: 2, paused: false };
    });
    expect(await ipc.watchStatus()).toEqual({ watching: 2, paused: false });
    await ipc.watchPause(true);
    expect(calls).toEqual([
      { cmd: "watch_status", args: {} },
      { cmd: "watch_pause", args: { paused: true } },
    ]);
  });
});
