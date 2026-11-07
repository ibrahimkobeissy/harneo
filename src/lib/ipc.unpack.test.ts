import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { libraryUnpackPlan, libraryUnpackPlugin } from "./ipc";

describe("unpacking a plugin", () => {
  it("asks for the plan and runs the unpack by artifact id", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return { parts: [] };
    });
    await libraryUnpackPlan("p1");
    await libraryUnpackPlugin("p1");
    expect(calls).toEqual([
      ["library_unpack_plan", { id: "p1" }],
      ["library_unpack_plugin", { id: "p1" }],
    ]);
  });
});
