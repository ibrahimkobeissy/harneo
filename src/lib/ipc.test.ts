import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import * as ipc from "./ipc";

interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

function record(result: unknown = null): Call[] {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    return result;
  });
  return calls;
}

afterEach(() => clearMocks());

describe("usage wrappers", () => {
  it("usageIndex, usageOf and usageSummary send their commands", async () => {
    const calls = record({ ok: true });
    expect(await ipc.usageIndex()).toEqual({ ok: true });
    expect(await ipc.usageOf("skill", "demo")).toEqual({ ok: true });
    expect(await ipc.usageSummary()).toEqual({ ok: true });
    expect(calls).toEqual([
      { cmd: "usage_index", args: {} },
      { cmd: "usage_of", args: { kind: "skill", name: "demo" } },
      { cmd: "usage_summary", args: {} },
    ]);
  });

  it("onUsageIndexProgress hands the event payload to the handler", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: unknown[] = [];
    const stop = await ipc.onUsageIndexProgress((p) => seen.push(p));
    await emit("usage-index-progress", { done: 1, total: 2 });
    expect(seen).toEqual([{ done: 1, total: 2 }]);
    stop();
    await emit("usage-index-progress", { done: 2, total: 2 });
    expect(seen).toHaveLength(1);
  });

  it("onBundlesDetectProgress hands the event payload to the handler", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: unknown[] = [];
    await ipc.onBundlesDetectProgress((p) => seen.push(p));
    await emit("bundles-detect-progress", { batch: 1 });
    expect(seen).toEqual([{ batch: 1 }]);
  });
});

describe("links, bundles and settings wrappers", () => {
  it("send the right command and argument keys", async () => {
    const calls = record("value");
    expect(await ipc.linksOf("a")).toBe("value");
    await ipc.linkAdd("a", "b", { type: "user" } as never, "why");
    await ipc.linkRemove("a", "b");
    await ipc.linkReject("a", "b");
    expect(await ipc.bundleOf("a")).toBe("value");
    expect(await ipc.settingsGet("k")).toBe("value");
    await ipc.settingsSet("k", { x: 1 });
    expect(calls).toEqual([
      { cmd: "links_of", args: { id: "a" } },
      {
        cmd: "link_add",
        args: { from: "a", to: "b", origin: { type: "user" }, reason: "why" },
      },
      { cmd: "link_remove", args: { from: "a", to: "b" } },
      { cmd: "link_reject", args: { from: "a", to: "b" } },
      { cmd: "bundle_of", args: { id: "a" } },
      { cmd: "settings_get", args: { key: "k" } },
      { cmd: "settings_set", args: { key: "k", value: { x: 1 } } },
    ]);
  });
});

describe("detection wrappers", () => {
  it("send the right command and argument keys", async () => {
    const calls = record("r");
    const preview = { sources: [] } as never;
    expect(await ipc.claudeCliStatus()).toBe("r");
    expect(await ipc.bundlesDetectPreview(preview, ["x"])).toBe("r");
    expect(await ipc.bundlesDetectLibrary(null)).toBe("r");
    expect(await ipc.bundlesDetectLibrary(["a"])).toBe("r");
    expect(calls).toEqual([
      { cmd: "claude_cli_status", args: {} },
      {
        cmd: "bundles_detect_preview",
        args: { preview, selection: ["x"] },
      },
      { cmd: "bundles_detect_library", args: { ids: null } },
      { cmd: "bundles_detect_library", args: { ids: ["a"] } },
    ]);
  });
});

describe("update wrappers", () => {
  it("send the right command and argument keys", async () => {
    const calls = record("r");
    expect(await ipc.updatesCheck()).toBe("r");
    await ipc.updatesCheck({ type: "source", id: "s" } as never);
    expect(await ipc.updateStatus("a")).toBe("r");
    expect(await ipc.updateDiff("a")).toBe("r");
    expect(await ipc.updateApply("a", "upstream" as never)).toBe("r");
    expect(await ipc.updateAll()).toBe("r");
    expect(calls).toEqual([
      { cmd: "updates_check", args: { scope: { type: "all" } } },
      {
        cmd: "updates_check",
        args: { scope: { type: "source", id: "s" } },
      },
      { cmd: "update_status", args: { id: "a" } },
      { cmd: "update_diff", args: { id: "a" } },
      { cmd: "update_apply", args: { id: "a", choice: "upstream" } },
      { cmd: "update_all", args: {} },
}
}
import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import {
  artifactSetManualOnly,
  profileClone,
  profileCreate,
  profileDelete,
  profileDescribe,
  profileFromCollection,
  profileGet,
  profileImportCommit,
  profileLayout,
  profileRename,
  profileResolve,
  profileSave,
} from "./ipc";

const spec = { items: ["a"], settings: null, mcp: [], manualOnly: {} };

describe("profile wrappers", () => {
  it("send ids as numbers with the command's argument names", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [
        "profile_create",
        "profile_clone",
        "profile_from_collection",
      ].includes(cmd)
        ? 5
        : null;
    });
    expect(await profileCreate("A", "d", spec)).toBe(5n);
    expect(await profileClone(5n, "B")).toBe(5n);
    await profileGet(5n);
    await profileGet(5n, 2);
    await profileSave(5n, spec);
    await profileRename(5n, "C");
    await profileDescribe(5n, "d2");
    await profileDelete(5n);
    await profileFromCollection(3n, "From");
    await profileResolve(spec);
    await profileLayout(spec);
    await profileImportCommit("/p.json");
    await profileImportCommit("/p.json", "N");
    await artifactSetManualOnly("a", true);
    expect(calls).toEqual([
      ["profile_create", { name: "A", description: "d", spec }],
      ["profile_clone", { id: 5, name: "B" }],
      ["profile_get", { id: 5, version: null }],
      ["profile_get", { id: 5, version: 2 }],
      ["profile_save", { id: 5, spec }],
      ["profile_rename", { id: 5, name: "C" }],
      ["profile_describe", { id: 5, description: "d2" }],
      ["profile_delete", { id: 5 }],
      ["profile_from_collection", { collectionId: 3, name: "From" }],
      ["profile_resolve", { spec }],
      ["profile_layout", { spec }],
      ["profile_import_commit", { path: "/p.json", name: null }],
      ["profile_import_commit", { path: "/p.json", name: "N" }],
      ["artifact_set_manual_only", { id: "a", manualOnly: true }],
    ]);
  });
});
