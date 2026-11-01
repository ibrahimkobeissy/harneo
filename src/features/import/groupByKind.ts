import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Candidate } from "../../bindings/Candidate";

/** Candidates grouped by kind, in the order kinds first appear. */
export function groupByKind(
  candidates: Candidate[],
): [ArtifactKind, Candidate[]][] {
  const groups = new Map<ArtifactKind, Candidate[]>();
  for (const c of candidates) {
    const list = groups.get(c.artifact.kind) ?? [];
    list.push(c);
    groups.set(c.artifact.kind, list);
  }
  return [...groups];
}
