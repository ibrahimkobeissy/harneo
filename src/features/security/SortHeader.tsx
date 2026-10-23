import { ArrowDown, ArrowUp } from "lucide-react";
import { useState } from "react";
import type { Dir } from "./model";
import styles from "./Security.module.css";

export interface Sort<K extends string> {
  key: K;
  dir: Dir;
}

/** Sort state for one table: clicking the active column flips it, another column starts at `first`. */
export function useSort<K extends string>(
  initial: Sort<K>,
  first: (column: K) => Dir = () => "asc",
) {
  const [sort, setSort] = useState<Sort<K>>(initial);
  const toggle = (column: K) =>
    setSort((cur) =>
      cur.key === column
        ? { key: column, dir: cur.dir === "asc" ? "desc" : "asc" }
        : { key: column, dir: first(column) },
    );
  return [sort, toggle] as const;
}

export function SortHeader<K extends string>({
  col,
  label,
  sort,
  onSort,
}: {
  col: K;
  label: string;
  sort: Sort<K>;
  onSort: (column: K) => void;
}) {
  const active = sort.key === col;
  const Arrow = sort.dir === "desc" ? ArrowDown : ArrowUp;
  return (
    <th
      scope="col"
      aria-sort={
        active ? (sort.dir === "desc" ? "descending" : "ascending") : "none"
      }
    >
      <button
        type="button"
        className={styles.sortBtn}
        onClick={() => onSort(col)}
      >
        {label}
        {active && <Arrow size={12} aria-hidden="true" />}
      </button>
    </th>
  );
}
