import type { Candidate } from "../../bindings/Candidate";
import { TrustMeter } from "../../ui";

/** The tier as the shared compact meter: name and score. */
export function TierLabel({ candidate }: { candidate: Candidate }) {
  return (
    <TrustMeter
      score={candidate.audit.score}
      tier={candidate.audit.tier}
      size="chip"
    />
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
