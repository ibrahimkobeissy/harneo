import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { PluginPart } from "../bindings/PluginPart";
import type { WholeReason } from "../bindings/WholeReason";

/**
 * The one place the UI knows what a plugin is: its kind, the mark of a remote-only entry, the
 * labels of what it holds and why it stays whole. The wording of a reason comes from the core
 * (`WholeReason::text`) and is carried with it.
 */

/** True when `kind` is a plugin (a whole one: its parts have their own kinds). */
export function isPlugin(kind: ArtifactKind | null | undefined): boolean {
  return kind === "plugin";
}

/** True when a plugin's path is a remote marketplace entry (`<file>#<entry>`): no files here. */
export function isRemoteOnlyPath(path: string): boolean {
  return path.includes("#");
}

/** Why a plugin stays whole: the reason, and the sentence the core words it with. */
export interface WholeNote {
  reason: WholeReason;
  text: string;
}

/** A kind of plugin component as a list names it. */
const PART_LABELS: Record<PluginPart, string> = {
  skills: "skills",
  agents: "agents",
  commands: "commands",
  outputStyles: "output styles",
  mcpServers: "MCP servers",
  hooks: "hooks",
  monitors: "monitors",
  lspServers: "LSP servers",
  mods: "mods",
  bin: "bin tools",
  themes: "themes",
  workflows: "workflows",
  settings: "settings",
};

/** What unpacking would lose ("hooks, mods"), or null when nothing comes only with the plugin. */
export function lostIfUnpacked(note: WholeNote | null): string | null {
  return note?.reason.type === "wholeOnlyParts"
    ? note.reason.parts.map((p) => PART_LABELS[p]).join(", ")
    : null;
}
