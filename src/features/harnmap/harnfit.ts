import type { FitCandidate } from "../../bindings/FitCandidate";
import { harnfitUrl } from "../../ui";

export { PREVIEW_PREFIX, verdictLabel, verdictTone } from "../../ui";

/** The route state an import screen passes along for a candidate that is not in the library. */
export interface FitRouteState {
  candidate?: FitCandidate;
  /** Where Discard goes back to. */
  from?: string;
  /** The candidate's name, for the header while Claude works. */
  name?: string;
}

/** The URL of the Harnfit screen: the key is encoded, the candidate of a preview rides in state. */
export const harnfitPath = harnfitUrl;

/** The candidate to run for a URL key and the route state, or null when it can't be run. */
export function candidateFor(
  key: string,
  state: FitRouteState | null,
): FitCandidate | null {
  if (state?.candidate) return state.candidate;
  return key.startsWith("lib:") ? { type: "library", id: key.slice(4) } : null;
}

/** The ways a Harnfit call fails that have a card of their own. */
export type FitProblem =
  "noMap" | "plugin" | "importFirst" | "alreadyIn" | "notFound";

/** Core's refusals reach the window as their message text; this maps them back. */
export function fitProblem(message: string): FitProblem | null {
  const text = message.toLowerCase();
  if (text.includes("has no map yet")) return "noMap";
  if (text.includes("take a plugin whole")) return "plugin";
  if (text.includes("import it into the library first")) return "importFirst";
  if (text.includes("is already in the profile")) return "alreadyIn";
  if (text.includes("no saved fit")) return "notFound";
  return null;
}

/** "4 Nov" for a fit's date. */
export function fitDate(createdAt: number): string {
  return new Date(createdAt * 1000).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
  });
}
