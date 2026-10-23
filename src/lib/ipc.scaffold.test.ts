import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { scaffoldCheck } from "./ipc";

describe("scaffold wrappers", () => {
  it("scaffoldCheck sends the target to scaffold_check", async () => {
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return null;
    });
    await scaffoldCheck("/work/app");
    expect(calls).toEqual([
      { cmd: "scaffold_check", args: { target: "/work/app" } },
    ]);
  });

  it("scaffoldCheck rejects with the core's message", async () => {
    mockIPC(() => {
      throw "/work/app has a harness";
    });
    await expect(scaffoldCheck("/work/app")).rejects.toBe(
      "/work/app has a harness",
    );
  });
});
