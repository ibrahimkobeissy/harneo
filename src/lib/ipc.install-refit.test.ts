import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import { refitCompare, refitPlan } from "./ipc";

describe("refit wrappers", () => {
  it("send ids as numbers, a missing version as null and the choices as given", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return "r";
    });
    expect(await refitCompare("/p", 3n)).toBe("r");
    expect(await refitCompare("/p", 3n, 2)).toBe("r");
    expect(await refitPlan("/p", 3n, { "CLAUDE.md": "keep" })).toBe("r");
    expect(await refitPlan("/p", 3n, {}, 2)).toBe("r");
    expect(calls).toEqual([
      ["refit_compare", { target: "/p", profileId: 3, version: null }],
      ["refit_compare", { target: "/p", profileId: 3, version: 2 }],
      [
        "refit_plan",
        {
          target: "/p",
          profileId: 3,
          version: null,
          choices: { "CLAUDE.md": "keep" },
        },
      ],
      ["refit_plan", { target: "/p", profileId: 3, version: 2, choices: {} }],
    ]);
  });

  it("reject with the command's error", async () => {
    mockIPC(() => {
      throw "not a folder";
    });
    await expect(refitCompare("/x", 1n)).rejects.toBe("not a folder");
    await expect(refitPlan("/x", 1n, {})).rejects.toBe("not a folder");
  });
});
