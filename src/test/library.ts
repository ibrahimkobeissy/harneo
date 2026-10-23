import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { LibraryItem } from "../bindings/LibraryItem";
import type { Page } from "../bindings/Page";

/** A listed artifact with sensible defaults, for tests that mock `library_list`. */
export function item(
  name: string,
  over: Partial<LibraryItem> = {},
  kind: ArtifactKind = "skill",
): LibraryItem {
  return {
    id: `id-${name}`,
    kind,
    name,
    description: `About ${name}`,
    source: { id: "s1", kind: "local", label: "agent-skills", branch: null },
    tags: [],
    importedAt: 1_700_000_000n,
    errorCount: 0,
    warningCount: 0,
    fileCount: 1,
    referenceCount: 0,
    resolvedReferences: 0,
    trustScore: null,
    tier: null,
    findingCount: 0,
    bundleSize: 0,
    update: null,
    usage: null,
    status: "current",
    ...over,
  };
}

/** A page holding `items`, with empty facets unless given. */
export function page(items: LibraryItem[], over: Partial<Page> = {}): Page {
  return {
    items,
    total: items.length,
    facets: {
      kinds: [],
      tags: [],
      sources: [],
      tiers: [],
      withFindings: 0,
      outdated: 0,
      statuses: [],
    },
    ...over,
    unfilteredTotal: over.unfilteredTotal ?? over.total ?? items.length,
  };
}
