import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import type { PartNeedRow } from "../../ui";
import type { PluginGroup } from "./plugins";

/**
 * The kept parts of a plugin that name a part the user dropped (ADR-008): a part still ticked
 * whose `<plugin>:<name>` call reaches a part that is excluded and not already in the library.
 * Nothing is decided here: each row offers to keep the needed part or drop both.
 */
export function partNeedRows(
  preview: Pick<Preview, "partRefs">,
  group: PluginGroup,
  excluded: ReadonlySet<string>,
  actions: {
    keep: (id: string) => void;
    drop: (id: string) => void;
  },
): PartNeedRow[] {
  const byId = new Map<string, Candidate>(group.parts.map((c) => [c.id, c]));
  const rows: PartNeedRow[] = [];
  for (const ref of preview.partRefs) {
    const from = byId.get(ref.from);
    const to = byId.get(ref.to);
    if (!from || !to) continue;
    if (excluded.has(from.id) || from.alreadyInLibrary) continue;
    if (!excluded.has(to.id) || to.alreadyInLibrary) continue;
    rows.push({
      id: `${from.id}:${to.id}`,
      part: from.artifact.name,
      kind: from.artifact.kind,
      needs: to.artifact.name,
      canKeep: true,
      onKeep: () => actions.keep(to.id),
      onDropBoth: () => actions.drop(from.id),
    });
  }
  return rows;
}
