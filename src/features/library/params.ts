import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Filter } from "../../bindings/Filter";
import type { Query } from "../../bindings/Query";
import type { SortKey } from "../../bindings/SortKey";
import type { Tier } from "../../bindings/Tier";
import { kinds } from "./kinds";

export type View = "cards" | "list";

/** Everything the screen shows, as kept in the URL query. */
export interface LibraryParams {
  view: View;
  sort: SortKey;
  descending: boolean;
  page: number;
  text: string;
  types: ArtifactKind[];
  tags: string[];
  sources: string[];
  /** Keep artifacts scored at least this; null is no limit. */
  trustMin: number | null;
  tiers: Tier[];
  hasFindings: boolean;
  /** The artifact open in the detail panel. */
  open: string | null;
  /** `trust`: the panel scrolls to its Trust section (Security links finding rows here). */
  focus: "trust" | null;
}

export const PER_PAGE: Record<View, number> = { cards: 24, list: 25 };

export const sortKeys: readonly SortKey[] = [
  "name",
  "kind",
  "source",
  "tagCount",
  "trust",
  "importedAt",
];

export const tiers: readonly Tier[] = [
  "trusted",
  "review",
  "risky",
  "dangerous",
];

export const sortLabels: Record<SortKey, string> = {
  name: "Name",
  kind: "Type",
  source: "Source",
  tagCount: "Tags",
  importedAt: "Imported",
  trust: "Trust",
  // A manual collection's own order (collection screens, L12); not offered on the library.
  position: "Manual order",
};

/** The first click on a column sorts newest first for dates and A to Z otherwise. */
export function defaultDescending(key: SortKey): boolean {
  // Highest trust first, like the dates' newest first.
  return key === "importedAt" || key === "trust";
}

/**
 * The sort a screen starts on: by name in the library, in manual order on a manual collection.
 * `position` is only accepted from the URL where it is the default.
 */
export type DefaultSort = "name" | "position";

export function parseParams(
  sp: URLSearchParams,
  fallback: DefaultSort = "name",
): LibraryParams {
  const sortParam = sp.get("sort");
  const accepted: readonly SortKey[] =
    fallback === "position" ? ["position", ...sortKeys] : sortKeys;
  const sort = accepted.find((k) => k === sortParam) ?? fallback;
  const dir = sp.get("dir");
  const trust = Number.parseInt(sp.get("trust") ?? "", 10);
  const page = Number.parseInt(sp.get("page") ?? "1", 10);
  return {
    view: sp.get("view") === "list" ? "list" : "cards",
    sort,
    descending: dir === "desc",
    page: Number.isFinite(page) && page > 0 ? page : 1,
    text: sp.get("q") ?? "",
    types: sp.getAll("type").filter((t): t is ArtifactKind => t in kinds),
    tags: sp.getAll("tag"),
    sources: sp.getAll("source"),
    trustMin: Number.isFinite(trust) && trust > 0 ? Math.min(100, trust) : null,
    tiers: sp
      .getAll("tier")
      .filter((t): t is Tier => tiers.includes(t as Tier)),
    hasFindings: sp.get("findings") === "1",
    open: sp.get("open") || null,
    focus: sp.get("open") && sp.get("focus") === "trust" ? "trust" : null,
  };
}

/** Writes only what differs from the defaults, so a bare `#/library` stays bare. */
export function toSearchParams(
  p: LibraryParams,
  fallback: DefaultSort = "name",
): URLSearchParams {
  const sp = new URLSearchParams();
  if (p.view !== "cards") sp.set("view", p.view);
  if (p.sort !== fallback || p.descending) {
    sp.set("sort", p.sort);
    sp.set("dir", p.descending ? "desc" : "asc");
  }
  if (p.page > 1) sp.set("page", String(p.page));
  if (p.text) sp.set("q", p.text);
  p.types.forEach((t) => sp.append("type", t));
  p.tags.forEach((t) => sp.append("tag", t));
  p.sources.forEach((s) => sp.append("source", s));
  if (p.trustMin !== null && p.trustMin > 0)
    sp.set("trust", String(p.trustMin));
  p.tiers.forEach((t) => sp.append("tier", t));
  if (p.hasFindings) sp.set("findings", "1");
  if (p.open) sp.set("open", p.open);
  if (p.open && p.focus) sp.set("focus", p.focus);
  return sp;
}

/** The filters as a `Filter`, the shape a smart collection stores as its rule. */
export function toFilter(p: LibraryParams): Filter {
  return {
    types: p.types,
    tags: p.tags,
    sources: p.sources,
    text: p.text.trim() === "" ? null : p.text,
    trustMin: p.trustMin !== null && p.trustMin > 0 ? p.trustMin : null,
    tiers: p.tiers,
    hasFindings: p.hasFindings,
  };
}

export function toQuery(p: LibraryParams): Query {
  const limit = PER_PAGE[p.view];
  return {
    filter: toFilter(p),
    sort: p.sort,
    descending: p.descending,
    offset: (p.page - 1) * limit,
    limit,
  };
}

export function hasFilters(p: LibraryParams): boolean {
  return (
    p.types.length + p.tags.length + p.sources.length + p.tiers.length > 0 ||
    p.trustMin !== null ||
    p.hasFindings ||
    p.text.trim() !== ""
  );
}
