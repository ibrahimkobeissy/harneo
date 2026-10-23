import { ShieldAlert } from "lucide-react";
import type { Candidate } from "../../bindings/Candidate";
import { severityLabels, severityOrder, tierLabels, tierOrder } from "../../ui";
import styles from "./Import.module.css";
import { TierLabel } from "./trust";

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

/** Review step: what the audit found in the artifacts that are about to be imported. */
export function AuditSummary({ chosen }: { chosen: Candidate[] }) {
  if (chosen.length === 0) return null;
  const tierCount = (t: (typeof tierOrder)[number]) =>
    chosen.filter((c) => c.audit.tier === t).length;
  const findings = chosen.flatMap((c) => c.audit.findings);
  const sevCount = (s: (typeof severityOrder)[number]) =>
    findings.filter((f) => f.severity === s).length;
  const reaching = chosen.filter(
    (c) => c.audit.reach.network.length + c.audit.reach.shell.length > 0,
  );
  const flagged = chosen.filter(
    (c) => c.audit.tier === "risky" || c.audit.tier === "dangerous",
  );
  return (
    <section aria-label="Security audit">
      <h2 className={styles.h}>
        Security audit{" "}
        <span>
          static checks on the files that will be copied; nothing was run
        </span>
      </h2>
      <div className={styles.auditStats}>
        <ul className={styles.stats} aria-label="Artifacts per tier">
          {tierOrder.map((t) => (
            <li key={t} className={styles.stat} data-tier={t}>
              {`${tierCount(t)} ${tierLabels[t]}`}
            </li>
          ))}
        </ul>
        <p className={styles.sec} aria-label="Findings per severity">
          {findings.length === 0
            ? "No findings."
            : `${plural(findings.length, "finding")}: ` +
              severityOrder
                .filter((s) => sevCount(s) > 0)
                .map((s) => `${sevCount(s)} ${severityLabels[s].toLowerCase()}`)
                .join(", ")}
        </p>
      </div>
      {reaching.length > 0 && (
        <>
          <h3 className={styles.subhead}>Reach network or shell</h3>
          <div className={styles.group}>
            {reaching.map((c) => (
              <div key={c.id} className={styles.row} role="group">
                <span className={styles.artifactName} title={c.artifact.name}>
                  {c.artifact.name}
                </span>
                <ul
                  className={styles.reach}
                  aria-label={`Reach of ${c.artifact.name}`}
                >
                  {c.audit.reach.network.map((h) => (
                    <li key={`n:${h}`} className={styles.reachItem}>
                      <span className={styles.reachKind}>network</span>
                      {h}
                    </li>
                  ))}
                  {c.audit.reach.shell.map((cmd) => (
                    <li key={`s:${cmd}`} className={styles.reachItem}>
                      <span className={styles.reachKind}>shell</span>
                      {cmd}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </>
      )}
      {flagged.length > 0 && (
        <>
          <h3 className={styles.subhead}>Flagged</h3>
          <div className={styles.group}>
            {flagged.map((c) => (
              <div
                key={c.id}
                className={styles.row}
                role="group"
                data-flag="true"
              >
                <ShieldAlert size={14} aria-hidden="true" />
                <span className={styles.artifactName} title={c.artifact.name}>
                  {c.artifact.name}
                </span>
                <TierLabel candidate={c} />
                <span className={`${styles.sec} ${styles.grow}`}>
                  {c.audit.findings.length === 0
                    ? "No findings listed."
                    : c.audit.findings
                        .slice(0, 2)
                        .map((f) => f.message)
                        .join(" ")}
                </span>
                <span className={styles.sec}>
                  {plural(c.audit.findings.length, "finding")}
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
