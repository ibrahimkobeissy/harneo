import { File, Folder } from "lucide-react";
import { type ReactNode, useRef, useState } from "react";
import type { WriteAction } from "../bindings/WriteAction";
import styles from "./FileTree.module.css";

export type FileMark = "new" | "replace" | "delete" | "kept";

export interface TreeFile {
  /** Relative path with forward slashes. */
  path: string;
  mark?: FileMark;
  /** Text shown in place of the mark's word, such as "+6 −2". */
  markText?: string;
}

/** The mark a plan action shows; a skipped path is not listed. */
export function markOf(action: WriteAction): FileMark | undefined {
  switch (action) {
    case "new":
      return "new";
    case "replace":
      return "replace";
    case "delete":
      return "delete";
    case "keep":
      return "kept";
    case "skip":
      return undefined;
  }
}

interface Dir {
  name: string;
  dirs: Map<string, Dir>;
  files: { name: string; file: TreeFile }[];
}

function build(files: readonly TreeFile[]): Dir {
  const root: Dir = { name: "", dirs: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split("/").filter(Boolean);
    const leaf = parts.pop();
    if (leaf === undefined) continue;
    let at = root;
    for (const part of parts) {
      let next = at.dirs.get(part);
      if (!next) {
        next = { name: part, dirs: new Map(), files: [] };
        at.dirs.set(part, next);
      }
      at = next;
    }
    at.files.push({ name: leaf, file });
  }
  return root;
}

const byName = (a: string, b: string) => a.localeCompare(b);

/** One visible row, in render order: `id` is the folder key or the file path. */
interface Row {
  id: string;
  parent: string | null;
  isDir: boolean;
}

function countFiles(dir: Dir): number {
  let n = dir.files.length;
  for (const d of dir.dirs.values()) n += countFiles(d);
  return n;
}

function sorted(dir: Dir) {
  return {
    dirs: [...dir.dirs.values()].sort((a, b) => byName(a.name, b.name)),
    files: [...dir.files].sort((a, b) => byName(a.name, b.name)),
  };
}

/** The rows a reader can reach: the children of a collapsed folder are left out. */
function visibleRows(
  dir: Dir,
  isCollapsed: (id: string) => boolean,
  prefix = "",
  parent: string | null = null,
): Row[] {
  const { dirs, files } = sorted(dir);
  const rows: Row[] = [];
  for (const d of dirs) {
    const id = `${prefix}${d.name}/`;
    rows.push({ id, parent, isDir: true });
    if (!isCollapsed(id)) {
      rows.push(...visibleRows(d, isCollapsed, id, id));
    }
  }
  for (const { file } of files) {
    rows.push({ id: file.path, parent, isDir: false });
  }
  return rows;
}

interface LevelProps {
  dir: Dir;
  depth: number;
  prefix: string;
  selected: string | null;
  active: string | null;
  isCollapsed: (id: string) => boolean;
  showCounts: boolean;
  action?: (path: string) => ReactNode;
  onSelect: (path: string) => void;
  onToggle: (id: string) => void;
  onFocusRow: (id: string) => void;
  onNavigate: (event: React.KeyboardEvent, id: string) => void;
}

function Level(props: LevelProps) {
  const { dir, depth, prefix, selected, active, isCollapsed } = props;
  const { dirs, files } = sorted(dir);
  return (
    <ul className={styles.list} role={depth === 0 ? "presentation" : "group"}>
      {dirs.map((d) => {
        const id = `${prefix}${d.name}/`;
        const open = !isCollapsed(id);
        return (
          <li key={`d:${id}`} role="presentation">
            <div
              role="treeitem"
              aria-expanded={open}
              aria-level={depth + 1}
              data-id={id}
              tabIndex={active === id ? 0 : -1}
              className={`${styles.node} ${styles.dir}`}
              style={{ "--depth": depth } as React.CSSProperties}
              onClick={() => props.onToggle(id)}
              onFocus={() => props.onFocusRow(id)}
              onKeyDown={(e) => props.onNavigate(e, id)}
            >
              <Folder size={14} aria-hidden="true" />
              <span className={styles.name}>{d.name}/</span>
              {props.showCounts && (
                <span className={styles.count}>{countFiles(d)}</span>
              )}
            </div>
            {open && <Level {...props} dir={d} depth={depth + 1} prefix={id} />}
          </li>
        );
      })}
      {files.map(({ name, file }) => (
        <li key={`f:${file.path}`} role="presentation">
          <div
            role="treeitem"
            aria-selected={selected === file.path}
            aria-level={depth + 1}
            data-id={file.path}
            tabIndex={active === file.path ? 0 : -1}
            className={`${styles.node} ${styles.file}`}
            style={{ "--depth": depth } as React.CSSProperties}
            aria-current={selected === file.path ? "true" : undefined}
            title={file.path}
            onClick={() => props.onSelect(file.path)}
            onFocus={() => props.onFocusRow(file.path)}
            onKeyDown={(e) => props.onNavigate(e, file.path)}
          >
            <File size={14} aria-hidden="true" />
            <span className={styles.name}>{name}</span>
            {file.mark && (
              <span className={styles.mark} data-mark={file.mark}>
                {file.markText ?? file.mark}
              </span>
            )}
            {props.action && (
              <span
                className={styles.action}
                onClick={(e) => e.stopPropagation()}
                onKeyDown={(e) => e.stopPropagation()}
              >
                {props.action(file.path)}
              </span>
            )}
          </div>
        </li>
      ))}
    </ul>
  );
}

/**
 * A folder tree of files, each with an optional mark; the selected file is `aria-current`.
 * Keyboard: Up/Down move, Left/Right collapse, expand or step to the parent/child, Home/End jump,
 * Enter or Space selects a file (or toggles a folder). One row is the tab stop (roving tabindex).
 */
export function FileTree({
  files,
  selected,
  onSelect,
  label = "Files",
  defaultCollapsed = false,
  showCounts = false,
  action,
}: {
  files: readonly TreeFile[];
  selected: string | null;
  onSelect: (path: string) => void;
  label?: string;
  /** Folders start collapsed instead of expanded. */
  defaultCollapsed?: boolean;
  /** Show the number of files under each folder. */
  showCounts?: boolean;
  /** Per-file controls shown at the end of a file row (clicks and keys stay off the row). */
  action?: (path: string) => ReactNode;
}) {
  const root = build(files);
  // Folders the reader flipped away from the default.
  const [flipped, setFlipped] = useState<ReadonlySet<string>>(new Set());
  const isCollapsed = (id: string) => defaultCollapsed !== flipped.has(id);
  const [focused, setFocused] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const rows = visibleRows(root, isCollapsed);
  // The one tab stop: the focused row, else the selected file, else the first row.
  const reachable = (id: string | null) => rows.some((r) => r.id === id);
  const active = reachable(focused)
    ? focused
    : reachable(selected)
      ? selected
      : (rows[0]?.id ?? null);

  function focusRow(id: string) {
    for (const node of ref.current?.querySelectorAll<HTMLElement>(
      "[data-id]",
    ) ?? []) {
      if (node.dataset.id === id) node.focus();
    }
  }

  function toggle(id: string) {
    setFlipped((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  function onNavigate(event: React.KeyboardEvent, id: string) {
    const at = rows.findIndex((r) => r.id === id);
    const row = rows[at];
    if (!row) return;
    const go = (to: Row | undefined) => {
      if (to) focusRow(to.id);
    };
    const open = row.isDir && !isCollapsed(id);
    switch (event.key) {
      case "ArrowDown":
        go(rows[at + 1]);
        break;
      case "ArrowUp":
        go(rows[at - 1]);
        break;
      case "Home":
        go(rows[0]);
        break;
      case "End":
        go(rows[rows.length - 1]);
        break;
      case "ArrowRight":
        if (row.isDir && !open) toggle(id);
        else if (open && rows[at + 1]?.parent === id) go(rows[at + 1]);
        break;
      case "ArrowLeft":
        if (open) toggle(id);
        else go(rows.find((r) => r.id === row.parent));
        break;
      case "Enter":
      case " ":
        if (row.isDir) toggle(id);
        else onSelect(id);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  return (
    <div className={styles.tree}>
      <div ref={ref} role="tree" aria-label={label}>
        <Level
          dir={root}
          depth={0}
          prefix=""
          selected={selected}
          active={active}
          isCollapsed={isCollapsed}
          showCounts={showCounts}
          action={action}
          onSelect={onSelect}
          onToggle={toggle}
          onFocusRow={setFocused}
          onNavigate={onNavigate}
        />
      </div>
    </div>
  );
}
