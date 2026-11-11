import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { Filter } from "../bindings/Filter";
import type { Status } from "../bindings/Status";
import type { Tier } from "../bindings/Tier";

/** An invocation filter value: `auto` keeps everything not manual only. */
export type Invocation = "auto" | "manual";

export const invocations: readonly Invocation[] = ["auto", "manual"];

export const invocationLabels: Record<Invocation, string> = {
  auto: "Auto",
  manual: "Manual only",
};

export const statusLabels: Record<Status, string> = {
  findings: "Findings",
  outdated: "Outdated",
  unused: "Unused",
  notUsedYet: "Not used yet",
  current: "Current",
};

/** The values the filter column shows and toggles. */
export interface FilterParams {
  types: ArtifactKind[];
  tags: string[];
  sources: string[];
  /** Keep artifacts scored at least this; null is no limit. */
  trustMin: number | null;
  tiers: Tier[];
  hasFindings: boolean;
  statuses: Status[];
  /** Both or none is no restriction. */
  invocation: Invocation[];
}

/** The filter column's groups that are plain lists of ticked values. */
export type FilterGroup =
  "types" | "tags" | "sources" | "tiers" | "statuses" | "invocation";

/** No filter set. */
export const noFilters: FilterParams = {
  types: [],
  tags: [],
  sources: [],
  trustMin: null,
  tiers: [],
  hasFindings: false,
  statuses: [],
  invocation: [],
};

/** How many filters are set, the count on the Filters button. */
export function activeFilterCount(p: FilterParams): number {
  return (
    p.types.length +
    p.tags.length +
    p.sources.length +
    p.tiers.length +
    p.statuses.length +
    p.invocation.length +
    (p.trustMin !== null ? 1 : 0) +
    (p.hasFindings ? 1 : 0)
  );
}

/** Ticks `value` in `group` if it is off, and unticks it if it is on. */
export function toggleFilter(
  p: FilterParams,
  group: FilterGroup,
  value: string,
): Partial<FilterParams> {
  const current = p[group] as string[];
  const next = current.includes(value)
    ? current.filter((v) => v !== value)
    : [...current, value];
  return { [group]: next } as Partial<FilterParams>;
}

/** Exactly one of Auto / Manual only narrows the list; both (or none) do not. */
function manualOnlyOf(invocation: Invocation[]): boolean | null {
  const auto = invocation.includes("auto");
  const manual = invocation.includes("manual");
  return auto === manual ? null : manual;
}

/** The filters and search text as a `Filter`, the shape `library_list` and smart collections take. */
export function filterOf(p: FilterParams, text: string): Filter {
  const manual = manualOnlyOf(p.invocation);
  return {
    types: p.types,
    tags: p.tags,
    sources: p.sources,
    text: text.trim() === "" ? null : text,
    trustMin: p.trustMin !== null && p.trustMin > 0 ? p.trustMin : null,
    tiers: p.tiers,
    hasFindings: p.hasFindings,
    outdated: false,
    statuses: p.statuses,
    ...(manual === null ? {} : { manualOnly: manual }),
  };
}
