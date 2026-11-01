import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import {
  fromPlugin,
  needsPlugin,
  partsRefusal,
  pluginGroups,
  remoteOnly,
  wholeRefusal,
} from "./plugins";

function cand(
  kind: Candidate["artifact"]["kind"],
  path: string,
  metadata: Record<string, unknown> = {},
): Candidate {
  return {
    id: path,
    artifact: { kind, name: path, path, metadata, diagnostics: [] },
  } as unknown as Candidate;
}

describe("plugin helpers", () => {
  it("reads the plugin of a part and ignores malformed metadata", () => {
    expect(
      fromPlugin(cand("skill", "a", { fromPlugin: { folder: "p" } })),
    ).toEqual({ name: "p", folder: "p" });
    expect(fromPlugin(cand("skill", "a", { fromPlugin: "x" }))).toBeNull();
    expect(
      fromPlugin(cand("skill", "a", { fromPlugin: { name: 1 } })),
    ).toBeNull();
    expect(fromPlugin(cand("skill", "a"))).toBeNull();
    expect(needsPlugin(cand("skill", "a"))).toBeNull();
  });

  it("groups a plugin's parts and flags a remote entry as whole only", () => {
    const groups = pluginGroups([
      cand("plugin", "remote.json#x"),
      cand("skill", "s", { fromPlugin: { name: "kit", folder: "kit" } }),
      cand("skill", "t", { fromPlugin: { name: "kit", folder: "kit" } }),
      cand("skill", "loose"),
    ]);
    expect(groups.map((g) => [g.folder, g.parts.length])).toEqual([
      ["remote.json#x", 0],
      ["kit", 2],
    ]);
    const [remote, kit] = groups;
    expect(remote && remoteOnly(remote)).toBe(true);
    expect(remote && partsRefusal(remote, undefined)).toMatch(/never fetched/);
    expect(kit && remoteOnly(kit)).toBe(false);
    expect(kit && wholeRefusal(kit, undefined)).toBeNull();
    expect(kit && partsRefusal(kit, undefined)).toBeNull();
  });

  it("gives the scan's reason when a plugin has nothing to pick", () => {
    const empty = cand("plugin", "hooksonly");
    empty.artifact.diagnostics.push({
      code: "plugin.no-parts",
      severity: "warning",
      file: "hooksonly/.claude-plugin/plugin.json",
      message: "nothing to pick, so it can only be taken whole",
    } as unknown as Candidate["artifact"]["diagnostics"][number]);
    const [group] = pluginGroups([empty]);
    expect(group && partsRefusal(group, undefined)).toBe(
      "nothing to pick, so it can only be taken whole",
    );
  });
});
