import type { Candidate } from "../../bindings/Candidate";
import type { PluginHold } from "../../bindings/PluginHold";
import type { PluginPart } from "../../bindings/PluginPart";
import type { WholeReason } from "../../bindings/WholeReason";

/** The error the scan puts on a part that only works inside its plugin. */
const NEEDS_PLUGIN = "plugin.part-needs-plugin";
/** The note the scan puts on a plugin asked to unpack that has nothing to pick. */
const NO_PARTS = "plugin.no-parts";

/** What comes only with the whole plugin, as the Choose step words it. */
export const WHOLE_ONLY =
  "Hooks, monitors, LSP servers, bin/ and the plugin's settings come only with the whole plugin.";

/** A plugin of the preview: whole as one row, or unpacked as its parts. */
export interface PluginGroup {
  name: string;
  /** The folder in the source: what the scan's `unpack` names. */
  folder: string;
  /** The plugin row, when the plugin is listed whole. */
  whole: Candidate | null;
  /** The parts, when it is unpacked. */
  parts: Candidate[];
}

/** The plugin an unpacked part came from, or null for any other candidate. */
export function fromPlugin(
  c: Candidate,
): { name: string; folder: string } | null {
  const from = c.artifact.metadata.fromPlugin;
  if (typeof from !== "object" || from === null) return null;
  const { name, folder } = from as { name?: unknown; folder?: unknown };
  if (typeof folder !== "string") return null;
  return { name: typeof name === "string" ? name : folder, folder };
}

/** The reason a part cannot leave its plugin, or null when it can. */
export function needsPlugin(c: Candidate): string | null {
  const found = c.artifact.diagnostics.find(
    (d) => d.severity === "error" && d.code === NEEDS_PLUGIN,
  );
  return found ? found.message : null;
}

/** The plugins of a preview in the order they appear, each once. */
export function pluginGroups(candidates: Candidate[]): PluginGroup[] {
  const groups = new Map<string, PluginGroup>();
  for (const c of candidates) {
    const part = fromPlugin(c);
    if (c.artifact.kind === "plugin" && part === null) {
      groups.set(c.artifact.path, {
        name: c.artifact.name,
        folder: c.artifact.path,
        whole: c,
        parts: [],
      });
    } else if (part !== null) {
      const group = groups.get(part.folder) ?? {
        name: part.name,
        folder: part.folder,
        whole: null,
        parts: [],
      };
      group.parts.push(c);
      groups.set(part.folder, group);
    }
  }
  return [...groups.values()];
}

/** A remote marketplace entry has no files here, so it can only be taken whole. */
export function remoteOnly(group: PluginGroup): boolean {
  return group.whole?.artifact.path.includes("#") === true;
}

/** Why the whole plugin cannot be imported, or null. Mirrors the core's refusal. */
export function wholeRefusal(
  group: PluginGroup,
  hold: PluginHold | undefined,
): string | null {
  return hold?.parts && !hold.whole
    ? `Parts of the plugin are in the library; remove them first to import the plugin ${group.name} whole.`
    : null;
}

/** Why its parts cannot be picked, or null. Mirrors the core's refusal. */
export function partsRefusal(
  group: PluginGroup,
  hold: PluginHold | undefined,
): string | null {
  if (remoteOnly(group)) {
    return "This marketplace entry points to another repo that is never fetched, so it can only be taken whole.";
  }
  const nothing = group.whole?.artifact.diagnostics.find(
    (d) => d.code === NO_PARTS,
  );
  if (nothing) return nothing.message;
  return hold?.whole
    ? `The plugin is in the library whole; remove the plugin ${group.name} first to import its parts.`
    : null;
}

/** A kind of plugin component as a sentence names it. Mirrors the core. */
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

function joinParts(parts: PluginPart[]): string {
  return parts.map((p) => PART_LABELS[p]).join(", ");
}

/** Why a plugin is proposed whole, as the core words it (`WholeReason::text`). */
export function wholeReasonText(reason: WholeReason): string {
  switch (reason.type) {
    case "wholeOnlyParts":
      return `Has ${joinParts(reason.parts)}: they only work inside the plugin.`;
    case "pluginRootTie":
      return `Its ${joinParts(reason.parts)} refer to files outside themselves: they only work inside the plugin.`;
    case "remoteOnly":
      return "Its files live in another repo that is never fetched, so it can only be taken whole.";
  }
}

/** What unpacking would lose ("hooks, mods"), or null when nothing comes only with the plugin. */
export function lostIfUnpacked(reason: WholeReason | null): string | null {
  return reason?.type === "wholeOnlyParts" ? joinParts(reason.parts) : null;
}
