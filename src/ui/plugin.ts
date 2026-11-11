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
      return `Has ${join(why.parts)}: they only work inside the plugin.`;
    case "pluginOnlyKey": {
      const { key } = why;
      return `Its manifest has \`${key}\`, which only means something inside the plugin.`;
    }
    case "unknownKey": {
      const { key } = why;
      return `Its manifest has \`${key}\`, which Harneo does not know, so it cannot tell a copy would work.`;
    }
    case "pluginVariable":
      return `Its ${join(why.parts)} use \`${why.variable}\`, which only resolves inside the plugin.`;
    case "namespacedSelfReference":
      return `Its ${join(why.parts)} call other parts of the plugin by their \`plugin:name\`, which a copy loses.`;
    case "agentField":
      return `An agent sets \`${why.field}\`, which Claude Code ignores in a plugin but runs when copied.`;
    case "noParts":
      return "It has no skills, agents, commands, output styles or MCP servers to unpack.";
    case "remoteOnly":
      return "Its files live in another repo that is never fetched, so it can only be taken whole.";
  }
}
