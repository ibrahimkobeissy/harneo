import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Query } from "../../bindings/Query";
import type { SortKey } from "../../bindings/SortKey";
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
}

export const PER_PAGE: Record<View, number> = { cards: 24, list: 25 };

export const sortKeys: readonly SortKey[] = [
  "name",
  "kind",
  "source",
  "tagCount",
  "importedAt",
];

export const sortLabels: Record<SortKey, string> = {
  name: "Name",
  kind: "Type",
  source: "Source",
  tagCount: "Tags",
  importedAt: "Imported",
};

/** The first click on a column sorts newest first for dates and A to Z otherwise. */
export function defaultDescending(key: SortKey): boolean {
  return key === "importedAt";
}

export function parseParams(sp: URLSearchParams): LibraryParams {
  const sortParam = sp.get("sort");
  const sort = sortKeys.find((k) => k === sortParam) ?? "name";
  const dir = sp.get("dir");
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
  };
}

/** Writes only what differs from the defaults, so a bare `#/library` stays bare. */
export function toSearchParams(p: LibraryParams): URLSearchParams {
  const sp = new URLSearchParams();
  if (p.view !== "cards") sp.set("view", p.view);
  if (p.sort !== "name" || p.descending) {
    sp.set("sort", p.sort);
    sp.set("dir", p.descending ? "desc" : "asc");
  }
  if (p.page > 1) sp.set("page", String(p.page));
  if (p.text) sp.set("q", p.text);
  p.types.forEach((t) => sp.append("type", t));
  p.tags.forEach((t) => sp.append("tag", t));
  p.sources.forEach((s) => sp.append("source", s));
  return sp;
}

export function toQuery(p: LibraryParams): Query {
  const limit = PER_PAGE[p.view];
  return {
    filter: {
      types: p.types,
      tags: p.tags,
      sources: p.sources,
      text: p.text.trim() === "" ? null : p.text,
    },
    sort: p.sort,
    descending: p.descending,
    offset: (p.page - 1) * limit,
    limit,
  };
}

export function hasFilters(p: LibraryParams): boolean {
  return (
    p.types.length + p.tags.length + p.sources.length > 0 ||
    p.text.trim() !== ""
  );
}
