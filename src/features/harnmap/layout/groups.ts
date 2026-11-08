/** One station in a stack: its depth is 0 for an ordinary node, 1 for a child, 2 for a grandchild. */
export interface Entry {
  id: string;
  depth: number;
  /** The parent drawn above it in the same stack, or null. */
  parent: string | null;
}

/** A parent with its children and theirs, in drawing order; an ordinary node is a group of one. */
export type Group = Entry[];

/** Two levels at most: command, skill, subagent. */
const MAX_DEPTH = 2;

/**
 * Splits the ids of one zone (already in map order) into groups. A child follows its parent;
 * a parent that is not in this zone (hidden, elsewhere) leaves its child as an ordinary node.
 */
export function groupsOf(
  ids: string[],
  parentOf: ReadonlyMap<string, string>,
): Group[] {
  const here = new Set(ids);
  const kids = new Map<string, string[]>();
  const roots: string[] = [];
  for (const id of ids) {
    const parent = parentOf.get(id);
    if (parent !== undefined && parent !== id && here.has(parent)) {
      kids.set(parent, [...(kids.get(parent) ?? []), id]);
    } else roots.push(id);
  }
  const seen = new Set<string>();
  const walk = (
    id: string,
    depth: number,
    parent: string | null,
    out: Group,
  ) => {
    if (seen.has(id)) return;
    seen.add(id);
    out.push({ id, depth, parent });
    for (const kid of kids.get(id) ?? []) {
      walk(kid, Math.min(depth + 1, MAX_DEPTH), id, out);
    }
  };
  const groups: Group[] = [];
  for (const id of [...roots, ...ids]) {
    const group: Group = [];
    walk(id, 0, null, group);
    if (group.length > 0) groups.push(group);
  }
  return groups;
}

/**
 * Splits a busy stack into at most two columns of about equal height. A group is never split
 * across the columns; a group taller than half the stack fills a column by itself.
 */
export function splitColumns(groups: Group[], maxStack: number): Entry[][] {
  const all = groups.flat();
  if (all.length <= maxStack) return [all];
  const rows = Math.ceil(all.length / 2);
  const first: Entry[] = [];
  let at = 0;
  while (at < groups.length) {
    const g = groups[at] ?? [];
    if (first.length > 0 && first.length + g.length > rows) break;
    first.push(...g);
    at++;
  }
  const second = groups.slice(at).flat();
  return second.length === 0 ? [first] : [first, second];
}
