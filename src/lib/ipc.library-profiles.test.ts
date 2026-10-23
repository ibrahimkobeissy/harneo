import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { artifactProfiles } from "./ipc";

describe("artifactProfiles", () => {
  it("asks for the profiles holding an artifact", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [{ id: 2, name: "Light", version: 3 }];
    });
    expect(await artifactProfiles("a")).toEqual([
      { id: 2, name: "Light", version: 3 },
    ]);
    expect(calls).toEqual([["artifact_profiles", { id: "a" }]]);
  });
});
