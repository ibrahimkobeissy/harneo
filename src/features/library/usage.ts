import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Status } from "../../bindings/Status";

/** Kinds whose use shows in Claude's session logs (mirrors `usage::is_tracked`). */
export const trackedKinds: readonly ArtifactKind[] = [
  "skill",
  "command",
  "agent",
  "mcp",
  "plugin",
];

export function isTracked(kind: ArtifactKind): boolean {
  return trackedKinds.includes(kind);
}

export { relativeDay } from "../../ui/dates";

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

/**
 * The status to show: until an index has completed, tracked artifacts all compute as unused,
 * which says nothing yet, so that one reads as current.
 */
export function shownStatus(item: LibraryItem, indexed: boolean): Status {
  return !indexed && item.status === "unused" ? "current" : item.status;
}
