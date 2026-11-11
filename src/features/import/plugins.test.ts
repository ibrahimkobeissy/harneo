import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { WholeReason } from "../../bindings/WholeReason";
import { wholeText } from "../../ui/plugin";
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

describe("why a plugin stays whole", () => {
  const say = (reason: WholeReason) => wholeText({ reason, text: "" });
  it("words every reason as the core does", () => {
    expect(say({ type: "wholeOnlyParts", parts: ["hooks", "mods"] })).toBe(
      "Has hooks and mods: they only work inside the plugin.",
    );
    expect(say({ type: "wholeOnlyParts", parts: ["hooks"] })).toBe(
      "Has hooks: they only work inside the plugin.",
    );
    expect(say({ type: "pluginOnlyKey", key: "userConfig" })).toBe(
      "Its manifest has `userConfig`, which only means something inside the plugin.",
    );
    expect(say({ type: "unknownKey", key: "x" })).toBe(
      "Its manifest has `x`, which Harneo does not know, so it cannot tell a copy would work.",
    );
    expect(
      say({
        type: "pluginVariable",
        variable: "${CLAUDE_PLUGIN_ROOT}",
        parts: ["skills", "agents", "commands"],
      }),
    ).toBe(
      "Its skills, agents and commands use `${CLAUDE_PLUGIN_ROOT}`, which only resolves inside the plugin.",
    );
    expect(say({ type: "namespacedSelfReference", parts: ["skills"] })).toBe(
      "Its skills call other parts of the plugin by their `plugin:name`, which a copy loses.",
    );
    expect(say({ type: "agentField", field: "hooks" })).toBe(
      "An agent sets `hooks`, which Claude Code ignores in a plugin but runs when copied.",
    );
    expect(say({ type: "noParts" })).toBe(
      "It has no skills, agents, commands, output styles or MCP servers to unpack.",
    );
    expect(say({ type: "remoteOnly" })).toBe(
      "Its files live in another repo that is never fetched, so it can only be taken whole.",
    );
  });

  it("prefers the sentence the core sent", () => {
    expect(wholeText({ reason: { type: "noParts" }, text: "From core." })).toBe(
      "From core.",
    );
  });
});
