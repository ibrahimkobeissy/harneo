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

/** `a`, `a and b`, `a, b and c`. */
function join(parts: PluginPart[]): string {
  const labels = parts.map((p) => PART_LABELS[p]);
  const last = labels.pop();
  if (last === undefined) return "";
  return labels.length === 0 ? last : `${labels.join(", ")} and ${last}`;
}

/** The reason as a sentence: the core's own (`WholeReason::text`), or the same words when absent. */
export function wholeText(note: WholeNote): string {
  if (note.text) return note.text;
  const why = note.reason;
  switch (why.type) {
    case "wholeOnlyParts":
      return `Has ${join(why.parts)}, which a plugin scaffolded under .claude/skills/ does not load, so it stays whole and installs from its marketplace.`;
    case "noParts":
      return "It has no skills, agents, commands or output styles to unpack.";
    case "remoteOnly":
      return "Its files live in another repo that is never fetched, so it can only be taken whole.";
  }
}
