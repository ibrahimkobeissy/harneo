import type { Candidate } from "../../bindings/Candidate";
import type { Diagnostic } from "../../bindings/Diagnostic";
import type { Origin } from "../../bindings/Origin";

/** The small label a row shows for where its artifact came from. */
export function originLabel(origin: Origin): string {
  switch (origin.type) {
    case "own":
      return "Yours";
    case "claudeSynced":
      return "Synced from your Claude account";
    case "devMod":
      return "Mod in development";
    case "marketplace":
      return origin.official
        ? "Anthropic marketplace"
        : `Marketplace: ${origin.name}`;
  }
}

/** Provided by Claude itself: synced from the account, or from the official marketplace. */
export function isClaudeProvided(origin: Origin): boolean {
  return (
    origin.type === "claudeSynced" ||
    (origin.type === "marketplace" && origin.official)
  );
}

export function claudeProvidedIds(candidates: Candidate[]): string[] {
  return candidates
    .filter((c) => !c.alreadyInLibrary && isClaudeProvided(c.artifact.origin))
    .map((c) => c.id);
}

/**
 * The `scan.managed-skipped` infos as one sentence, or null when there are none. The core words
 * each as "<n> skill|mod folder(s) not scanned".
 */
export function managedNote(diagnostics: Diagnostic[]): string | null {
  let skills = 0;
  let mods = 0;
  let found = false;
  for (const d of diagnostics) {
    if (d.code !== "scan.managed-skipped") continue;
    found = true;
    const m = /(\d+) (skill|mod) folder/.exec(d.message);
    if (!m) continue;
    if (m[2] === "mod") mods += Number(m[1]);
    else skills += Number(m[1]);
  }
  if (!found) return null;
  const parts: string[] = [];
  if (skills > 0 || mods === 0)
    parts.push(`${skills} synced skill${skills === 1 ? "" : "s"}`);
  if (mods > 0)
    parts.push(`${mods} mod${mods === 1 ? "" : "s"} in development`);
  return `${parts.join(" and ")} skipped: Claude manages them`;
}
