import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Layout } from "../../bindings/Layout";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { SourceRecord } from "../../bindings/SourceRecord";

/** MCP env values are `${NAME}` placeholders only (decision 4 of `spec/profiles.md`). */
export const placeholderPattern = /^\$\{[A-Z0-9_]+\}$/;

export const emptySpec: ProfileSpec = {
  items: [],
  settings: null,
  mcp: [],
  manualOnly: {},
};

/** The profile's name when the user leaves it empty. */
export const defaultName = "Untitled profile";

/** Equal keys mean equal specs; map order does not matter. */
export function specKey(spec: ProfileSpec): string {
  const sorted = (o: Record<string, unknown>) =>
    Object.entries(o).sort(([a], [b]) => a.localeCompare(b));
  return JSON.stringify([
    spec.items,
    spec.settings,
    spec.mcp.map((m) => [m.artifact, sorted(m.env)]),
    sorted(spec.manualOnly),
  ]);
}

export function isEmpty(spec: ProfileSpec): boolean {
  return (
    spec.items.length === 0 && spec.settings === null && spec.mcp.length === 0
  );
}

export function addItem(spec: ProfileSpec, id: string): ProfileSpec {
  return spec.items.includes(id)
    ? spec
    : { ...spec, items: [...spec.items, id] };
}

/** Drops `ids` from the picked items, with their manual-only overrides. */
export function dropItems(spec: ProfileSpec, ids: string[]): ProfileSpec {
  const manualOnly = Object.fromEntries(
    Object.entries(spec.manualOnly).filter(([id]) => !ids.includes(id)),
  );
  return {
    ...spec,
    items: spec.items.filter((id) => !ids.includes(id)),
    manualOnly,
  };
}

export function setManualOnly(
  spec: ProfileSpec,
  id: string,
  value: boolean,
): ProfileSpec {
  return { ...spec, manualOnly: { ...spec.manualOnly, [id]: value } };
}

/** Adds a server with one `${KEY}` placeholder per env key. */
export function addMcp(
  spec: ProfileSpec,
  artifact: string,
  envKeys: string[],
): ProfileSpec {
  if (spec.mcp.some((m) => m.artifact === artifact)) return spec;
  const env = Object.fromEntries(envKeys.map((k) => [k, `\${${k}}`]));
  return { ...spec, mcp: [...spec.mcp, { artifact, env }] };
}

export function removeMcp(spec: ProfileSpec, artifact: string): ProfileSpec {
  return { ...spec, mcp: spec.mcp.filter((m) => m.artifact !== artifact) };
}

export function setEnv(
  spec: ProfileSpec,
  artifact: string,
  key: string,
  value: string,
): ProfileSpec {
  return {
    ...spec,
    mcp: spec.mcp.map((m) =>
      m.artifact === artifact ? { ...m, env: { ...m.env, [key]: value } } : m,
    ),
  };
}

/** The env variable names an MCP server's own definition lists (its values are placeholders). */
export function mcpEnvKeys(detail: ArtifactDetail): string[] {
  const env = detail.artifact.metadata["env"];
  const keys =
    env && typeof env === "object" && !Array.isArray(env)
      ? Object.keys(env)
      : [];
  return keys.length > 0 ? keys : ["API_TOKEN"];
}

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function listLength(value: unknown): number {
  return Array.isArray(value) ? value.length : 0;
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** "deny 6 · ask 2 · allow 4 · 1 hook · status line · style: terse" (prototype `security.js`). */
export function settingsSummary(detail: ArtifactDetail): string {
  const meta = detail.artifact.metadata;
  const permissions = record(meta["permissions"]);
  let hooks = 0;
  for (const groups of Object.values(record(meta["hooks"]))) {
    if (!Array.isArray(groups)) continue;
    for (const group of groups) hooks += listLength(record(group)["hooks"]);
  }
  const bits = [
    `deny ${listLength(permissions["deny"])}`,
    `ask ${listLength(permissions["ask"])}`,
    `allow ${listLength(permissions["allow"])}`,
    plural(hooks, "hook"),
  ];
  if (meta["statusLine"]) bits.push("status line");
  if (typeof meta["outputStyle"] === "string") {
    bits.push(`style: ${meta["outputStyle"]}`);
  }
  const plugins = Object.keys(record(meta["enabledPlugins"])).length;
  if (plugins > 0) bits.push(plural(plugins, "plugin"));
  return bits.join(" · ");
}

/** The template's `settings.json` as stored (literal secrets already replaced). */
export function settingsJson(detail: ArtifactDetail): string {
  return JSON.stringify(detail.artifact.metadata, null, 2);
}

/** "name" for a local folder, "owner/repo"-style for a git source. */
export function sourceLabel(source: SourceRecord): string {
  const parts = source.location.replace(/\/+$/, "").split("/");
  if (source.kind === "git") return parts.slice(-2).join("/");
  return parts[parts.length - 1] ?? source.location;
}

export interface TreeRow {
  /** The full path for a file, the folder path for a folder. */
  path: string;
  name: string;
  depth: number;
  file: boolean;
  conflict: boolean;
}

/** The layout as indented rows: folders before files at each level, each sorted by name. */
export function treeRows(layout: Layout): TreeRow[] {
  interface Node {
    name: string;
    path: string;
    file: boolean;
    kids: Map<string, Node>;
  }
  const root: Node = { name: "", path: "", file: false, kids: new Map() };
  const conflicted = new Set(layout.conflicts.map((c) => c.path));
  for (const { path } of layout.paths) {
    let node = root;
    const parts = path.split("/");
    parts.forEach((part, i) => {
      let next = node.kids.get(part);
      if (!next) {
        next = {
          name: part,
          path: parts.slice(0, i + 1).join("/"),
          file: i === parts.length - 1,
          kids: new Map(),
        };
        node.kids.set(part, next);
      }
      node = next;
    });
  }
  const rows: TreeRow[] = [];
  const walk = (node: Node, depth: number) => {
    const kids = [...node.kids.values()].sort(
      (a, b) => Number(a.file) - Number(b.file) || a.name.localeCompare(b.name),
    );
    for (const kid of kids) {
      rows.push({
        path: kid.path,
        name: kid.name,
        depth,
        file: kid.file,
        conflict: kid.file && conflicted.has(kid.path),
      });
      walk(kid, depth + 1);
    }
  };
  walk(root, 0);
  return rows;
}
