import {
  Bot,
  FileText,
  Package,
  Palette,
  Plug,
  Scale,
  SlidersHorizontal,
  Sparkles,
  Terminal,
  Workflow,
  type LucideIcon,
} from "lucide-react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { TagTint } from "../../ui";

export interface KindMeta {
  label: string;
  plural: string;
  icon: LucideIcon;
  tint: string;
}

/** Label, icon and tint per artifact kind, as in the prototype's type table. */
export const kinds: Record<ArtifactKind, KindMeta> = {
  skill: { label: "Skill", plural: "Skills", icon: Sparkles, tint: "blue" },
  agent: { label: "Subagent", plural: "Subagents", icon: Bot, tint: "violet" },
  command: {
    label: "Command",
    plural: "Commands",
    icon: Terminal,
    tint: "cyan",
  },
  rule: { label: "Rule", plural: "Rules", icon: Scale, tint: "slate" },
  mcp: {
    label: "MCP server",
    plural: "MCP servers",
    icon: Plug,
    tint: "orange",
  },
  plugin: { label: "Plugin", plural: "Plugins", icon: Package, tint: "teal" },
  settings: {
    label: "Settings",
    plural: "Settings",
    icon: SlidersHorizontal,
    tint: "slate",
  },
  outputStyle: {
    label: "Output style",
    plural: "Output styles",
    icon: Palette,
    tint: "pink",
  },
  workflow: {
    label: "Workflow",
    plural: "Workflows",
    icon: Workflow,
    tint: "indigo",
  },
  claudeMd: {
    label: "CLAUDE.md",
    plural: "CLAUDE.md",
    icon: FileText,
    tint: "amber",
  },
};

const tagTints: readonly string[] = [
  "cyan",
  "sky",
  "indigo",
  "violet",
  "fuchsia",
  "stone",
  "slate",
  "red",
  "orange",
  "amber",
  "lime",
  "emerald",
  "teal",
  "pink",
];

/** A tag's stored color as a `Tag` tint; anything unknown falls back to slate. */
export function tagTint(color: string): TagTint {
  return tagTints.includes(color) ? (color as TagTint) : "slate";
}
