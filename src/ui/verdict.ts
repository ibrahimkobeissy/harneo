import type { Verdict } from "../bindings/Verdict";
import type { BadgeTone } from "./Badge";

/** A Harnfit verdict in words. */
export function verdictLabel(verdict: Verdict): string {
  switch (verdict) {
    case "fits":
      return "Fits";
    case "fitsWithChanges":
      return "Fits with changes";
    case "conflicts":
      return "Conflicts";
  }
}

/** Green, amber, red: state colours, never blue. */
export function verdictTone(verdict: Verdict): BadgeTone {
  switch (verdict) {
    case "fits":
      return "ok";
    case "fitsWithChanges":
      return "warn";
    case "conflicts":
      return "bad";
  }
}

/** The Harnfit screen's address for a candidate key (`lib:<id>`), key URL-encoded. */
export function harnfitUrl(profile: bigint | number, key: string): string {
  return `/profiles/${String(profile)}/harnfit/${encodeURIComponent(key)}`;
}

/** The URL segment of an import-preview candidate: its key is only known once the fit has run. */
export const PREVIEW_PREFIX = "preview:";
