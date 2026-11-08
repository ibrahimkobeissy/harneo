import type { DrawFailure } from "../../bindings/DrawFailure";
import type { MapView } from "../../bindings/MapView";
import type { TemplateChange } from "../../bindings/TemplateChange";
import type { MapData } from "./types";

/** The setting that holds the model Harnmap draws with. */
export const MODEL_KEY = "harnmap.model";
export type DrawModel = "opus" | "sonnet";

/** Seconds the replay of the placements takes at most, and the gap between two artifacts. */
export const REPLAY_TOTAL_MS = 3000;
export const REPLAY_GAP_MS = 110;
/** How long the finished replay holds before the result shows. */
export const REPLAY_HOLD_MS = 400;

export function modelLabel(model: string): string {
  return model === "sonnet" ? "Sonnet" : model === "opus" ? "Opus" : model;
}

export function toModel(value: unknown): DrawModel {
  return value === "sonnet" ? "sonnet" : "opus";
}

/** The map canvas's input from a map view. */
export function toMapData(view: MapView): MapData {
  return {
    doc: view.doc,
    artifacts: view.artifacts,
    facts: view.facts,
    bundles: view.bundles,
    hooks: view.hooks,
  };
}

/** The gap between two artifacts in the replay: 0.11 s, squeezed so that it ends in ~3 s. */
export function replayGap(total: number): number {
  return Math.max(
    1,
    Math.min(REPLAY_GAP_MS, Math.floor(REPLAY_TOTAL_MS / Math.max(total, 1))),
  );
}

/** "1:05" for the honest wait's clock. */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** "52 s" or "1 min 12 s". */
export function took(ms: number): string {
  const s = Math.round(ms / 1000);
  return s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`;
}

/** What a rejected `harnmap_draw` carried, as a `DrawFailure`. */
export function asFailure(error: unknown): DrawFailure {
  if (
    typeof error === "object" &&
    error !== null &&
    "type" in error &&
    typeof error.type === "string"
  ) {
    return error as DrawFailure;
  }
  return { type: "other", message: String(error) };
}

export interface ChangeLine {
  mark: string;
  text: string;
  /** Artifact names to show in mono after the text. */
  names?: string[];
}

/** The computed changes from the template, in plain words. */
export function changeLines(
  changes: TemplateChange[],
  stageName: (id: string) => string,
  unplacedNames: string[],
): ChangeLine[] {
  const kept = changes.filter((c) => c.type === "stageKept").length;
  const lines: ChangeLine[] = [];
  const renamed = changes.flatMap((c) =>
    c.type === "stageRenamed" ? [c] : [],
  );
  const added = changes.flatMap((c) => (c.type === "stageAdded" ? [c] : []));
  const removed = changes.flatMap((c) =>
    c.type === "stageRemoved" ? [c] : [],
  );
  const plural = (n: number, word: string) =>
    `${n} ${word}${n === 1 ? "" : "s"}`;
  lines.push({
    mark: "=",
    text: `Kept ${plural(kept, "stage")}. Renamed ${renamed.length === 0 ? "none" : renamed.length}, added ${added.length === 0 ? "none" : added.length}.`,
  });
  for (const c of renamed)
    lines.push({ mark: "~", text: `Renamed ${c.from} to ${c.to}.` });
  for (const c of added)
    lines.push({ mark: "+", text: `Added the stage ${c.name}.` });
  for (const c of removed)
    lines.push({ mark: "−", text: `Removed the stage ${c.name}.` });
  let loopNoted = false;
  for (const c of changes) {
    if (c.type === "loopAdded") {
      // One line says what a loop is; the loops below it carry only their names.
      if (!loopNoted) {
        lines.push({
          mark: "↺",
          text: "Loops are Claude’s reading, not enforced.",
        });
        loopNoted = true;
      }
      lines.push({
        mark: "↺",
        text: `The loop from ${stageName(c.from)} to ${stageName(c.to)}${c.label ? ` (${c.label})` : ""}.`,
      });
    }
    if (c.type === "unplaced" && c.count > 0)
      lines.push({
        mark: "○",
        text: `Left ${plural(c.count, "artifact")} unplaced${unplacedNames.length > 0 ? ":" : "."}`,
        names: unplacedNames,
      });
  }
  return lines;
}
