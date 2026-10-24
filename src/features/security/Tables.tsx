import { ChevronRight } from "lucide-react";
import { Fragment, useState } from "react";
import type { Reach } from "../../bindings/Reach";
import { OpenArtifactButton } from "../../lib/artifactPanel";
import {
  AnyNote,
  FindingTitle,
  hasAny,
  SeverityBadge,
  severityLabels,
  severityRank,
  TrustMeter,
} from "../../ui";
import {
  groupFindings,
  kindLabel,
  reachKeys,
  reaches,
  sortRows,
  type Audited,
  type FindingGroup,
  type ReachKey,
} from "./model";
import { SortHeader, useSort } from "./SortHeader";
import styles from "./Security.module.css";

export function ArtifactLink({ a }: { a: Audited }) {
  return (
    <OpenArtifactButton className={styles.link} id={a.id} focus="trust">
      {a.name}
    </OpenArtifactButton>
  );
}

type FindingCol = "severity" | "artifact" | "type" | "findings" | "trust";

/** "2 critical, 1 high" for the worst severities of a group, plain text for screen readers too. */
function countsLabel(g: FindingGroup): string {
  const n = (s: string) => g.findings.filter((f) => f.severity === s).length;
  return (["critical", "high", "medium", "low"] as const)
    .filter((s) => n(s) > 0)
    .map((s) => `${n(s)} ${severityLabels[s].toLowerCase()}`)
    .join(", ");
}

export function FindingsTable({ list }: { list: readonly Audited[] }) {
  const [sort, onSort] = useSort<FindingCol>(
    { key: "severity", dir: "desc" },
    (c) => (c === "severity" ? "desc" : "asc"),
  );
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const toggle = (id: string) =>
    setOpen((cur) => {
      const next = new Set(cur);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const rows = sortRows(groupFindings(list), sort.dir, (g) => {
    switch (sort.key) {
      case "severity":
        // Within a severity the least trusted artifact comes first in descending order.
        return severityRank[g.worst] * 1000 - g.artifact.audit.score;
      case "artifact":
        return g.artifact.name;
      case "type":
        return kindLabel[g.artifact.kind];
      case "findings":
        return g.findings.length;
      case "trust":
        return g.artifact.audit.score;
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
              col="findings"
              label="Findings"
              sort={sort}
              onSort={onSort}
            />
            <SortHeader col="trust" label="Trust" sort={sort} onSort={onSort} />
          </tr>
        </thead>
        <tbody>
          {rows.map((g) => {
            const a = g.artifact;
            const expanded = open.has(a.id);
            return (
              <Fragment key={a.id}>
                <tr
                  className={g.worst === "critical" ? styles.flag : undefined}
                >
                  <td>
                    <SeverityBadge severity={g.worst} />
                  </td>
                  <td>
                    <ArtifactLink a={a} />
                  </td>
                  <td>{kindLabel[a.kind]}</td>
                  <td>
                    <button
                      type="button"
                      className={styles.expand}
                      aria-expanded={expanded}
                      onClick={() => toggle(a.id)}
                    >
                      <ChevronRight size={14} aria-hidden="true" />
                      <b>{g.findings.length}</b>
                      <span className={styles.counts}>{countsLabel(g)}</span>
                      <span className={styles.srOnly}>
                        {expanded ? "Hide" : "Show"} findings of {a.name}
                      </span>
                    </button>
                  </td>
                  <td>
                    <TrustMeter score={a.audit.score} tier={a.audit.tier} />
                  </td>
                </tr>
                {expanded &&
                  g.findings.map((f, i) => (
                    <tr
                      key={`${f.rule}:${f.file}:${f.line ?? 0}:${i}`}
                      className={styles.sub}
                    >
                      <td>
                        <SeverityBadge severity={f.severity} />
                      </td>
                      <td colSpan={3}>
                        <span className={styles.rule}>
                          <FindingTitle rule={f.rule} />
                        </span>
                        <span className={styles.msg}>{f.message}</span>
                        <span className={styles.where}>
                          {f.file}
                          {f.line !== null ? `:${f.line}` : ""}
                        </span>
                        <code className={styles.evidence} title={f.evidence}>
                          {f.evidence}
                        </code>
                      </td>
                      <td>
                        <OpenArtifactButton
                          className={styles.link}
                          id={a.id}
                          focus="trust"
                          aria-label={`View ${a.name} trust`}
                        >
                          View
                        </OpenArtifactButton>
                      </td>
                    </tr>
                  ))}
              </Fragment>
            );
          })}
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
      className={hasAny(list) ? styles.hot : undefined}
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
      {rows.some((a) => reachKeys.some((k) => hasAny(a.audit.reach[k]))) && (
        <AnyNote className={styles.note} />
      )}
    </div>
  );
}
