import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import {
  installApply,
  libraryImport,
  libraryPreview,
  profileExport,
  profileImportCommit,
  profileImportPreview,
  refitPlan,
  removalPlan,
  switchPlan,
  driftReapplyPlan,
} from "./ipc";

function record(answer: unknown) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return answer;
  });
  return calls;
}

describe("plans and previews stay in the backend", () => {
  it("install_apply and library_import send a token, never the plan or preview", async () => {
    const calls = record(null);
    const plan = { token: "p-tok", target: "/work", writes: [] } as never;
    const preview = { token: "v-tok", candidates: [] } as never;
    await installApply(plan);
    await libraryImport(preview, ["a"], { a: [] });
    await libraryImport(preview, ["a"], {}, [["a", "b", "why"]]);
    expect(calls).toEqual([
      { cmd: "install_apply", args: { token: "p-tok" } },
      {
        cmd: "library_import",
        args: { token: "v-tok", selection: ["a"], tags: { a: [] } },
      },
      {
        cmd: "library_import",
        args: {
          token: "v-tok",
          selection: ["a"],
          tags: {},
          links: [["a", "b", "why"]],
        },
      },
    ]);
  });

  it("plan and preview commands pass their arguments through", async () => {
    const calls = record({ token: "t" });
    expect(await libraryPreview({ kind: "local", path: "/s" })).toEqual({
      token: "t",
    });
    await refitPlan("/p", 1n, {});
    await switchPlan(2n, 1n);
    await removalPlan(2n);
    await driftReapplyPlan(2n, ["a"]);
    expect(calls.map((c) => c.cmd)).toEqual([
      "library_preview",
      "refit_plan",
      "switch_plan",
      "removal_plan",
      "drift_reapply_plan",
    ]);
  });

  it("profile export and import take no path from the window", async () => {
    const calls = record(null);
    expect(await profileExport(3n, "x.harneo-profile.json")).toBeNull();
    await profileExport(3n, "x.harneo-profile.json", 2, {
      includeHistory: false,
      includeFits: true,
    });
    expect(await profileImportPreview()).toBeNull();
    await profileImportCommit("f-tok");
    expect(calls).toEqual([
      {
        cmd: "profile_export",
        args: {
          id: 3,
          version: null,
          defaultName: "x.harneo-profile.json",
          includeHistory: true,
          includeFits: true,
        },
      },
      {
        cmd: "profile_export",
        args: {
          id: 3,
          version: 2,
          defaultName: "x.harneo-profile.json",
          includeHistory: false,
          includeFits: true,
        },
      },
      { cmd: "profile_import_preview", args: {} },
      { cmd: "profile_import_commit", args: { token: "f-tok", name: null } },
    ]);
  });
});
