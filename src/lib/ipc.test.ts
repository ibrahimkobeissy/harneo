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

describe("audit phase 2 wrappers", () => {
  it("send the right command and argument keys", async () => {
    const calls = record(true);
    expect(await ipc.auditConfirmations()).toBe(true);
    await ipc.auditConfirm(["a"]);
    await ipc.auditConfirmAll();
    expect(await ipc.auditConfirmSettingGet()).toBe(true);
    await ipc.auditConfirmSettingSet(false);
    expect(calls).toEqual([
      { cmd: "audit_confirmations", args: {} },
      { cmd: "audit_confirm", args: { ids: ["a"] } },
      { cmd: "audit_confirm_all", args: {} },
      { cmd: "audit_confirm_setting_get", args: {} },
      { cmd: "audit_confirm_setting_set", args: { enabled: false } },
    ]);
  });

  it("onAuditConfirmProgress hands the event payload to the handler", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: unknown[] = [];
    await ipc.onAuditConfirmProgress((p) => seen.push(p));
    await emit("audit-confirm-progress", { done: 1, total: 2 });
    expect(seen).toEqual([{ done: 1, total: 2 }]);
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
    const preview = { token: "tk", sources: [] } as never;
    expect(await ipc.claudeCliStatus()).toBe("r");
    expect(await ipc.bundlesDetectPreview(preview, ["x"], false)).toBe("r");
    expect(await ipc.bundlesDetectLibrary(null, true)).toBe("r");
    expect(await ipc.bundlesDetectLibrary(["a"], false)).toBe("r");
    expect(calls).toEqual([
      { cmd: "claude_cli_status", args: {} },
      {
        cmd: "bundles_detect_preview",
        args: { token: "tk", selection: ["x"], userInitiated: false },
      },
      {
        cmd: "bundles_detect_library",
        args: { ids: null, userInitiated: true },
      },
      {
        cmd: "bundles_detect_library",
        args: { ids: ["a"], userInitiated: false },
      },
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
    ]);
  });
});

describe("install wrappers", () => {
  it("send the right command and argument keys", async () => {
    const calls = record("r");
    const plan = { token: "tk", target: "/p" } as never;
    expect(await ipc.scaffoldPlan("/p", 3n)).toBe("r");
    await ipc.scaffoldPlan("/p", 3n, 2);
    expect(await ipc.installApply(plan)).toBe("r");
    expect(await ipc.projectsList()).toBe("r");
    expect(await ipc.projectGet(4n)).toBe("r");
    expect(await ipc.projectFiles(4n)).toBe("r");
    expect(await ipc.projectFileContent(4n, "a.md")).toBe("r");
    expect(calls).toEqual([
      {
        cmd: "scaffold_plan",
        args: { target: "/p", profileId: 3, version: null },
      },
      {
        cmd: "scaffold_plan",
        args: { target: "/p", profileId: 3, version: 2 },
      },
      { cmd: "install_apply", args: { token: "tk" } },
      { cmd: "projects_list", args: {} },
      { cmd: "project_get", args: { id: 4 } },
      { cmd: "project_files", args: { id: 4 } },
      { cmd: "project_file_content", args: { id: 4, path: "a.md" } },
    ]);
  });
});
