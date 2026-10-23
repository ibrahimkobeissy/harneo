import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import {
  addItem,
  addMcp,
  dropItems,
  emptySpec,
  isEmpty,
  mcpEnvKeys,
  removeMcp,
  setEnv,
  setManualOnly,
  settingsJson,
  settingsSummary,
  sourceLabel,
  specKey,
  treeRows,
} from "./spec";

function detail(metadata: Record<string, unknown>): ArtifactDetail {
  return { artifact: { metadata } } as unknown as ArtifactDetail;
}

describe("spec helpers", () => {
  it("compares specs without regard to map order", () => {
    const a: ProfileSpec = {
      ...emptySpec,
      items: ["x"],
      manualOnly: { a: true, b: false },
      mcp: [{ artifact: "m", env: { A: "${A}", B: "${B}" } }],
    };
    const b: ProfileSpec = {
      ...a,
      manualOnly: { b: false, a: true },
      mcp: [{ artifact: "m", env: { B: "${B}", A: "${A}" } }],
    };
    expect(specKey(a)).toBe(specKey(b));
    expect(specKey(a)).not.toBe(specKey(addItem(a, "y")));
    expect(isEmpty(emptySpec)).toBe(true);
    expect(isEmpty({ ...emptySpec, settings: "s" })).toBe(false);
  });

  it("adds each item once and prunes the overrides of dropped ones", () => {
    const once = addItem(addItem(emptySpec, "a"), "a");
    expect(once.items).toEqual(["a"]);
    const withOverride = setManualOnly(addItem(once, "b"), "a", true);
    const dropped = dropItems(withOverride, ["a"]);
    expect(dropped.items).toEqual(["b"]);
    expect(dropped.manualOnly).toEqual({});
  });

  it("adds, edits and removes servers", () => {
    const added = addMcp(emptySpec, "m", ["TOKEN"]);
    expect(added.mcp[0]?.env).toEqual({ TOKEN: "${TOKEN}" });
    expect(addMcp(added, "m", ["OTHER"])).toBe(added);
    const edited = setEnv(
      setEnv(added, "m", "TOKEN", "${X}"),
      "n",
      "T",
      "${Y}",
    );
    expect(edited.mcp[0]?.env).toEqual({ TOKEN: "${X}" });
    expect(removeMcp(edited, "m").mcp).toEqual([]);
  });

  it("reads env keys from an MCP server, with a generic one as the fallback", () => {
    expect(mcpEnvKeys(detail({ env: { A: "${A}", B: "${B}" } }))).toEqual([
      "A",
      "B",
    ]);
    expect(mcpEnvKeys(detail({ env: [] }))).toEqual(["API_TOKEN"]);
    expect(mcpEnvKeys(detail({}))).toEqual(["API_TOKEN"]);
  });

  it("summarises a settings template", () => {
    expect(settingsSummary(detail({}))).toBe(
      "deny 0 · ask 0 · allow 0 · 0 hooks",
    );
    expect(
      settingsSummary(
        detail({
          permissions: { deny: [1], ask: "no", allow: [] },
          hooks: {
            Stop: [{ hooks: [{}, {}] }],
            Odd: "nope",
          },
          outputStyle: "terse",
          enabledPlugins: { a: true, b: true },
        }),
      ),
    ).toBe("deny 1 · ask 0 · allow 0 · 2 hooks · style: terse · 2 plugins");
    expect(settingsJson(detail({ model: "m" }))).toBe('{\n  "model": "m"\n}');
  });

  it("labels sources", () => {
    const base = {
      id: "s",
      branch: null,
      subfolder: null,
      commitSha: null,
    };
    expect(
      sourceLabel({
        ...base,
        kind: "git",
        location: "https://github.com/a/b/",
      }),
    ).toBe("a/b");
    expect(
      sourceLabel({ ...base, kind: "local", location: "/home/u/.claude" }),
    ).toBe(".claude");
  });

  it("lays the tree out with folders first and flags conflicts", () => {
    const from = { type: "generated", generated: "harnessMd" } as const;
    const rows = treeRows({
      paths: [
        { path: "HARNESS.md", from },
        { path: ".claude/skills/b/SKILL.md", from },
        { path: ".claude/skills/a.md", from },
      ],
      conflicts: [{ path: ".claude/skills/a.md", artifacts: ["1", "2"] }],
    });
    expect(rows.map((r) => `${r.depth}:${r.name}`)).toEqual([
      "0:.claude",
      "1:skills",
      "2:b",
      "3:SKILL.md",
      "2:a.md",
      "0:HARNESS.md",
    ]);
    expect(rows.find((r) => r.name === "a.md")?.conflict).toBe(true);
    expect(rows.find((r) => r.name === "HARNESS.md")?.conflict).toBe(false);
  });
});
