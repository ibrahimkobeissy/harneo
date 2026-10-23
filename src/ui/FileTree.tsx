import { File, Folder } from "lucide-react";
import type { WriteAction } from "../bindings/WriteAction";
import styles from "./FileTree.module.css";

export type FileMark = "new" | "replace" | "delete" | "kept";

export interface TreeFile {
  /** Relative path with forward slashes. */
  path: string;
  mark?: FileMark;
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

function Level({
  dir,
  depth,
  prefix,
  selected,
  onSelect,
}: {
  dir: Dir;
  depth: number;
  prefix: string;
  selected: string | null;
  onSelect: (path: string) => void;
}) {
  const dirs = [...dir.dirs.values()].sort((a, b) => byName(a.name, b.name));
  const files = [...dir.files].sort((a, b) => byName(a.name, b.name));
  return (
    <ul className={styles.list}>
      {dirs.map((d) => (
        <li key={`d:${prefix}${d.name}`}>
          <div
            className={`${styles.node} ${styles.dir}`}
            style={{ "--depth": depth } as React.CSSProperties}
          >
            <Folder size={14} aria-hidden="true" />
            <span className={styles.name}>{d.name}/</span>
          </div>
          <Level
            dir={d}
            depth={depth + 1}
            prefix={`${prefix}${d.name}/`}
            selected={selected}
            onSelect={onSelect}
          />
        </li>
      ))}
      {files.map(({ name, file }) => (
        <li key={`f:${file.path}`}>
          <button
            type="button"
            className={styles.node}
            style={{ "--depth": depth } as React.CSSProperties}
            aria-current={selected === file.path ? "true" : undefined}
            title={file.path}
            onClick={() => onSelect(file.path)}
          >
            <File size={14} aria-hidden="true" />
            <span className={styles.name}>{name}</span>
            {file.mark && (
              <span className={styles.mark} data-mark={file.mark}>
                {file.mark}
              </span>
            )}
          </button>
        </li>
      ))}
    </ul>
  );
}

/** A folder tree of files, each with an optional mark; the selected file is `aria-current`. */
export function FileTree({
  files,
  selected,
  onSelect,
  label = "Files",
}: {
  files: readonly TreeFile[];
  selected: string | null;
  onSelect: (path: string) => void;
  label?: string;
}) {
  return (
    <nav className={styles.tree} aria-label={label}>
      <Level
        dir={build(files)}
        depth={0}
        prefix=""
        selected={selected}
        onSelect={onSelect}
      />
    </nav>
  );
}
