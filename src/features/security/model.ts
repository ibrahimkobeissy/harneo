import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import type { Audit } from "../../bindings/Audit";
import type { Finding } from "../../bindings/Finding";
import type { Reach } from "../../bindings/Reach";
import type { Tier } from "../../bindings/Tier";

/** An artifact that has a stored audit. */
export interface Audited extends ArtifactAudit {
  audit: Audit;
}

export function isAudited(a: ArtifactAudit): a is Audited {
  return a.audit !== null;
}

/** Ten buckets of ten points; 100 counts with 90 to 99. */
export function histogram(list: readonly Audited[]): number[] {
  const out = Array.from({ length: 10 }, () => 0);
  for (const a of list) {
    const i = Math.min(9, Math.floor(a.audit.score / 10));
    out[i] = (out[i] ?? 0) + 1;
  }
  return out;
}

/** The tier a bucket falls in (the tier boundaries are multiples of ten). */
export function tierOfBucket(i: number): Tier {
  if (i >= 8) return "trusted";
  if (i >= 6) return "review";
  if (i >= 4) return "risky";
  return "dangerous";
}

export interface FindingRow {
  artifact: Audited;
  finding: Finding;
}

export function findingRows(list: readonly Audited[]): FindingRow[] {
  return list.flatMap((artifact) =>
    artifact.audit.findings.map((finding) => ({ artifact, finding })),
  );
}

export type ReachKey = "files" | "shell" | "network" | "env";

export const reachKeys: readonly ReachKey[] = [
  "files",
  "shell",
  "network",
  "env",
];

export const reachTiles: readonly { key: ReachKey; label: string }[] = [
  { key: "network", label: "network hosts" },
  { key: "shell", label: "run shell commands" },
  { key: "env", label: "read env vars" },
  { key: "files", label: "touch absolute or sensitive files" },
];

export function reaches(reach: Reach, key: ReachKey): boolean {
  return reach[key].length > 0;
}

export function reachesAnything(reach: Reach): boolean {
  return reachKeys.some((k) => reaches(reach, k));
}

export type Dir = "asc" | "desc";

/** Sorts a copy by `value` (numbers or strings) in `dir`; ties keep their order. */
export function sortRows<T>(
  rows: readonly T[],
  dir: Dir,
  value: (row: T) => number | string,
): T[] {
  const sign = dir === "asc" ? 1 : -1;
  return rows
    .map((row, i) => ({ row, i, v: value(row) }))
    .sort((a, b) => {
      const c =
        typeof a.v === "number" && typeof b.v === "number"
          ? a.v - b.v
          : String(a.v).localeCompare(String(b.v));
      return c !== 0 ? sign * c : a.i - b.i;
    })
    .map((x) => x.row);
}

/** The reader-facing name of each artifact type. */
export const kindLabel: Record<ArtifactKind, string> = {
  claudeMd: "CLAUDE.md",
  rule: "Rule",
  skill: "Skill",
  command: "Command",
  agent: "Subagent",
  settings: "Settings",
  mcp: "MCP server",
  outputStyle: "Output style",
  workflow: "Workflow",
  plugin: "Plugin",
};
