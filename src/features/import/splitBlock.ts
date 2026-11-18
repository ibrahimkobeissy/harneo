import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import { pluginGroups } from "./plugins";

/**
 * Why a plugin set to Pick parts cannot be imported (ADR-008, amended 2026-11-18, temporary):
 * the supporting files of a part come only from Claude's answer.
 */
export type SplitCase =
  /** Claude was not asked: the setting is off or no answer is stored. */
  | { kind: "notAsked" }
  /** The call failed: an error, or an answer that did not parse. */
  | { kind: "failed"; message: string }
  /** The answer does not cover these kept parts (as the row shows them). */
  | { kind: "incomplete"; missing: string[] };

export interface SplitBlock {
  /** The plugin's folder: what the scan's `unpack` and `whole` name. */
  folder: string;
  name: string;
  why: SplitCase;
}

/** A part as its row names it: commands carry their slash. */
export function partLabel(c: Candidate): string {
  return c.artifact.kind === "command"
    ? `/${c.artifact.name}`
    : c.artifact.name;
}

/**
 * The plugins set to Pick parts that block Continue: one entry per plugin with a kept part that
 * would be imported, in the order the preview lists them. `failure` is the last run's error.
 */
export function splitBlocks(
  preview: Pick<Preview, "candidates" | "partNeeds" | "needsBy">,
  excluded: ReadonlySet<string>,
  failure: string | null,
): SplitBlock[] {
  const answered = new Set(preview.partNeeds.map((p) => p.path));
  const out: SplitBlock[] = [];
  for (const g of pluginGroups(preview.candidates)) {
    const kept = g.parts.filter(
      (c) => !c.alreadyInLibrary && !excluded.has(c.id),
    );
    if (kept.length === 0) continue;
    const base = { folder: g.folder, name: g.name };
    if (failure !== null) {
      out.push({ ...base, why: { kind: "failed", message: failure } });
    } else if (!preview.needsBy) {
      out.push({ ...base, why: { kind: "notAsked" } });
    } else {
      const missing = kept
        .filter((c) => !answered.has(c.artifact.path))
        .map(partLabel);
      if (missing.length > 0) {
        out.push({ ...base, why: { kind: "incomplete", missing } });
      }
    }
  }
  return out;
}
