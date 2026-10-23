import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import type { Audit } from "../../bindings/Audit";
import type { Finding } from "../../bindings/Finding";
import type { Reach } from "../../bindings/Reach";
import type { AuditSeverity } from "../../bindings/AuditSeverity";
import { severityRank } from "../../ui/audit";
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

/** Bar height as a share of the tallest bucket: square-root scaled, so a bucket of 3 stays visible next to one of 26. Empty is 0. */
export function barShare(n: number, max: number): number {
  if (n <= 0 || max <= 0) return 0;
  return Math.max(0.18, Math.sqrt(n / max));
}

/** One artifact with all its findings, most serious first. */
export interface FindingGroup {
  artifact: Audited;
  findings: Finding[];
  worst: AuditSeverity;
}

/** The artifacts that have findings, each with its findings sorted most serious first. */
export function groupFindings(list: readonly Audited[]): FindingGroup[] {
  return list.flatMap((artifact) => {
    const findings = [...artifact.audit.findings].sort(
      (a, b) => severityRank[b.severity] - severityRank[a.severity],
    );
    const first = findings[0];
    return first ? [{ artifact, findings, worst: first.severity }] : [];
  });
}

/** Where a re-audit moved an artifact's score. `before` is null for a never-audited artifact. */
export interface ScoreChange {
  id: string;
  name: string;
  before: number | null;
  after: number;
  tier: Tier;
}

/** The artifacts whose score differs between two reads of the audits (compared by id). */
export function scoreChanges(
  before: readonly ArtifactAudit[],
  after: readonly ArtifactAudit[],
): ScoreChange[] {
  const old = new Map(before.map((a) => [a.id, a.audit?.score ?? null]));
  return after.flatMap((a) => {
    if (a.audit === null) return [];
    const was = old.get(a.id) ?? null;
    return was === a.audit.score
      ? []
      : [
          {
            id: a.id,
            name: a.name,
            before: was,
            after: a.audit.score,
            tier: a.audit.tier,
          },
        ];
  });
}

export type ReachKey = "files" | "shell" | "network" | "env";

export const reachKeys: readonly ReachKey[] = [
  "files",
  "shell",
  "network",
  "env",
];

export interface ReachTile {
  key: ReachKey;
  /** The phrase after "N artifacts" (plural verb), then the form after "1 artifact". */
  label: string;
  singular: string;
  /** What the distinct entries are called, for the secondary line. */
  unit: [one: string, many: string];
}

export const reachTiles: readonly ReachTile[] = [
  {
    key: "network",
    label: "reach the network",
    singular: "reaches the network",
    unit: ["host", "hosts"],
  },
  {
    key: "shell",
    label: "run shell commands",
    singular: "runs shell commands",
    unit: ["command", "commands"],
  },
  {
    key: "env",
    label: "read env vars",
    singular: "reads env vars",
    unit: ["variable", "variables"],
  },
  {
    key: "files",
    label: "touch absolute or sensitive files",
    singular: "touches absolute or sensitive files",
    unit: ["path", "paths"],
  },
];

/** The secondary line of a reach tile: distinct entries across the artifacts, and whether any is `*`. */
export function reachDetail(list: readonly Audited[], tile: ReachTile): string {
  const all = list.flatMap((a) => a.audit.reach[tile.key]);
  const named = new Set(all.filter((x) => x !== "*")).size;
  const any = all.includes("*");
  if (named === 0) return any ? "any" : "none";
  const text = `${named} ${named === 1 ? tile.unit[0] : tile.unit[1]}`;
  return any ? `${text}, and any` : text;
}

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
