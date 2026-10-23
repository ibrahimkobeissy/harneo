import { Link } from "react-router";
import type { Reach } from "../../bindings/Reach";
import { Badge, type BadgeTone } from "../../ui";
import {
  findingRows,
  kindLabel,
  reachKeys,
  reaches,
  severityLabel,
  severityRank,
  sortRows,
  type Audited,
  type ReachKey,
} from "./model";
import { SortHeader, useSort } from "./SortHeader";
import styles from "./Security.module.css";
import { TrustMeter } from "./TrustMeter";

const tone: Record<string, BadgeTone> = {
  critical: "bad",
  high: "bad",
  medium: "warn",
  low: "neutral",
};

function ArtifactLink({ a }: { a: Audited }) {
  return (
    <Link className={styles.link} to={`/library?open=${a.id}`}>
      {a.name}
    </Link>
  );
}

type FindingCol =
  "severity" | "artifact" | "type" | "finding" | "evidence" | "trust";

export function FindingsTable({ list }: { list: readonly Audited[] }) {
  const [sort, onSort] = useSort<FindingCol>(
    { key: "severity", dir: "desc" },
    (c) => (c === "severity" ? "desc" : "asc"),
  );
  const rows = sortRows(findingRows(list), sort.dir, (r) => {
    switch (sort.key) {
      case "severity":
        // Within a severity the least trusted artifact comes first in descending order.
        return severityRank[r.finding.severity] * 1000 - r.artifact.audit.score;
      case "artifact":
        return r.artifact.name;
      case "type":
        return kindLabel[r.artifact.kind];
      case "finding":
        return r.finding.rule;
      case "evidence":
        return r.finding.evidence;
      case "trust":
        return r.artifact.audit.score;
    }
  });
  return (
    <div className={styles.scroll}>
      <table className={`${styles.table} ${styles.findings}`}>
        <caption className={styles.srOnly}>Findings</caption>
        <thead>
          <tr>
            <SortHeader
              col="severity"
              label="Severity"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader
              col="artifact"
              label="Artifact"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader col="type" label="Type" sort={sort} onSort={onSort} />
            <SortHeader
              col="finding"
              label="Finding"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader
              col="evidence"
              label="Evidence"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader col="trust" label="Trust" sort={sort} onSort={onSort} />
          </tr>
        </thead>
        <tbody>
          {rows.map(({ artifact: a, finding: f }, i) => (
            <tr
              key={`${a.id}:${f.rule}:${f.file}:${f.line ?? 0}:${i}`}
              className={
                f.severity === "critical" || f.severity === "high"
                  ? styles.flag
                  : undefined
              }
            >
              <td>
                <Badge tone={tone[f.severity] ?? "neutral"}>
                  {severityLabel[f.severity]}
                </Badge>
              </td>
              <td>
                <ArtifactLink a={a} />
              </td>
              <td>{kindLabel[a.kind]}</td>
              <td>
                <span className={styles.rule}>{f.rule}</span>
                <span className={styles.msg}>{f.message}</span>
                <span className={styles.where}>
                  {f.file}
                  {f.line !== null ? `:${f.line}` : ""}
                </span>
              </td>
              <td className={styles.mono}>
                <span className={styles.ell} title={f.evidence}>
                  {f.evidence}
                </span>
              </td>
              <td>
                <TrustMeter score={a.audit.score} tier={a.audit.tier} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

type ReachCol = ReachKey | "artifact" | "trust";

const reachHeads: readonly { col: ReachCol; label: string }[] = [
  { col: "artifact", label: "Artifact" },
  { col: "files", label: "Files" },
  { col: "shell", label: "Shell" },
  { col: "network", label: "Network" },
  { col: "env", label: "Env vars" },
  { col: "trust", label: "Trust" },
];

function ReachCell({ reach, k }: { reach: Reach; k: ReachKey }) {
  const list = reach[k];
  if (!reaches(reach, k)) return <span className={styles.none}>none</span>;
  const shown = list.slice(0, 2).join(", ");
  const more = list.length > 2 ? ` +${list.length - 2}` : "";
  return (
    <span
      className={list.includes("*") ? styles.hot : undefined}
      title={list.join(", ")}
    >
      {shown}
      {more}
    </span>
  );
}

export function ReachTable({ list }: { list: readonly Audited[] }) {
  const [sort, onSort] = useSort<ReachCol>({ key: "trust", dir: "asc" });
  const rows = sortRows(list, sort.dir, (a) => {
    if (sort.key === "artifact") return a.name;
    if (sort.key === "trust") return a.audit.score;
    return a.audit.reach[sort.key].length;
  });
  return (
    <div className={styles.scroll}>
      <table className={`${styles.table} ${styles.reachTable}`}>
        <caption className={styles.srOnly}>Reach</caption>
        <thead>
          <tr>
            {reachHeads.map((h) => (
              <SortHeader
                key={h.col}
                col={h.col}
                label={h.label}
                sort={sort}
                onSort={onSort}
              />
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((a) => (
            <tr key={a.id}>
              <td>
                <span className={styles.nameCell}>
                  <ArtifactLink a={a} />
                  <span className={styles.kind}>{kindLabel[a.kind]}</span>
                </span>
              </td>
              {reachKeys.map((k) => (
                <td key={k} className={styles.mono}>
                  <ReachCell reach={a.audit.reach} k={k} />
                </td>
              ))}
              <td>
                <TrustMeter score={a.audit.score} tier={a.audit.tier} />
              </td>
            </tr>
          ))}
          {rows.length === 0 && (
            <tr>
              <td colSpan={6} className={styles.muted}>
                None.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
