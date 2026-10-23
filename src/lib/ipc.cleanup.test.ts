import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { cleanupList, pruneApply, prunePlan } from "./ipc";

describe("cleanup and prune wrappers", () => {
  it("call the commands with the artifact id", async () => {
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return "r";
    });
    expect(await cleanupList()).toBe("r");
    expect(await prunePlan("a1")).toBe("r");
    expect(await pruneApply("a1")).toBe("r");
    expect(calls).toEqual([
      { cmd: "cleanup_list", args: {} },
      { cmd: "prune_plan", args: { artifact: "a1" } },
      { cmd: "prune_apply", args: { artifact: "a1" } },
    ]);
  });

  it("reject with the backend's message", async () => {
    mockIPC(() => {
      throw "not in the library";
    });
    await expect(cleanupList()).rejects.toBe("not in the library");
    await expect(prunePlan("x")).rejects.toBe("not in the library");
    await expect(pruneApply("x")).rejects.toBe("not in the library");
  });
});
