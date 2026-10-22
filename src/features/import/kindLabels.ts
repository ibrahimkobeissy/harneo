import type { ArtifactKind } from "../../bindings/ArtifactKind";

/**
 * Plural group titles per kind. Features may not import each other, so the Library's richer
 * kind table (icons, tints) is not shared here; hoist it to a shared module if both need it.
 */
export const kindPlurals: Record<ArtifactKind, string> = {
  skill: "Skills",
  agent: "Subagents",
  command: "Commands",
  rule: "Rules",
  mcp: "MCP servers",
  plugin: "Plugins",
  settings: "Settings",
  outputStyle: "Output styles",
  workflow: "Workflows",
  claudeMd: "CLAUDE.md",
};
