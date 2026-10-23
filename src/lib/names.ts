import { libraryDetail } from "./ipc";

/**
 * Display names for `ids`: those in `known` as they are, the rest looked up in the library.
 * An id that cannot be read keeps showing as itself.
 */
export async function resolveNames(
  ids: readonly string[],
  known: ReadonlyMap<string, string> = new Map(),
): Promise<Map<string, string>> {
  const names = new Map(known);
  await Promise.all(
    [...new Set(ids)]
      .filter((id) => !names.has(id))
      .map(async (id) => {
        try {
          const detail = await libraryDetail(id);
          if (detail?.artifact?.name) names.set(id, detail.artifact.name);
        } catch {
          // The id stays as its own label.
        }
      }),
  );
  return names;
}
