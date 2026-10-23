import type { AuditSeverity } from "../../bindings/AuditSeverity";
import type { Candidate } from "../../bindings/Candidate";
import type { Tier } from "../../bindings/Tier";
import styles from "./Import.module.css";

export const tierNames: Record<Tier, string> = {
  trusted: "Trusted",
  review: "Review",
  risky: "Risky",
  dangerous: "Dangerous",
};

export const tierOrder: Tier[] = ["trusted", "review", "risky", "dangerous"];

/** Most serious first. */
export const severityOrder: AuditSeverity[] = [
  "critical",
  "high",
  "medium",
  "low",
];

/** The tier as a small coloured label with the score. */
export function TierLabel({ candidate }: { candidate: Candidate }) {
  const { tier, score } = candidate.audit;
  return (
    <span
      className={styles.tier}
      data-tier={tier}
      title={`Trust score ${score} of 100`}
    >
      {tierNames[tier]}
      <span className={styles.tierScore}>{score}</span>
    </span>
  );
}

/** The rule of the first critical finding, if the candidate has one. */
export function criticalRule(candidate: Candidate): string | null {
  return (
    candidate.audit.findings.find((f) => f.severity === "critical")?.rule ??
    null
  );
}

/** Ids of the not-yet-imported candidates that start excluded for a critical finding. */
export function criticalIds(candidates: Candidate[]): string[] {
  return candidates
    .filter((c) => !c.alreadyInLibrary && criticalRule(c) !== null)
    .map((c) => c.id);
}
