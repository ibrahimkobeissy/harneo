import type { Candidate } from "../../bindings/Candidate";

/** How a candidate was placed, as the Choose step words it; `reason` is Claude's, when it placed it. */
export interface Placement {
  label: string;
  reason: string | null;
}

/** Reads `metadata.layoutBy` (and `layoutManifest`, `layoutReason`); null when the scan did not say. */
export function placementOf(candidate: Candidate): Placement | null {
  const meta = candidate.artifact.metadata;
  switch (meta.layoutBy) {
    case "manifest":
      return {
        label:
          meta.layoutManifest === "marketplace.json"
            ? "from marketplace.json"
            : "from plugin.json",
        reason: null,
      };
    case "convention":
      return { label: "recognised folder", reason: null };
    case "claude":
      return {
        label: "suggested by Claude",
        reason:
          typeof meta.layoutReason === "string" && meta.layoutReason !== ""
            ? meta.layoutReason
            : null,
      };
    default:
      return null;
  }
}
