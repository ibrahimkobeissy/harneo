import { ArrowDown, ArrowUp } from "lucide-react";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { SortKey } from "../../bindings/SortKey";
import { Findings, importedDate, sourceText, TagChips } from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import styles from "./Library.module.css";
import { sortKeys, sortLabels } from "./params";

export function ItemTable({
  items,
  sort,
  descending,
  onSort,
  onOpen,
}: {
  items: LibraryItem[];
  sort: SortKey;
  descending: boolean;
  onSort: (key: SortKey) => void;
  onOpen: (item: LibraryItem) => void;
}) {
  const Arrow = descending ? ArrowDown : ArrowUp;
  return (
    <table className={styles.table}>
      <thead>
        <tr>
          {sortKeys.map((key) => {
            const active = key === sort;
            return (
              <th
                key={key}
                scope="col"
                aria-sort={
                  active ? (descending ? "descending" : "ascending") : "none"
                }
              >
                <button
                  type="button"
                  className={styles.sortBtn}
                  onClick={() => onSort(key)}
                >
                  {sortLabels[key]}
                  {active && <Arrow size={12} aria-hidden="true" />}
                </button>
              </th>
            );
          })}
        </tr>
      </thead>
      <tbody>
        {items.map((item) => (
          <tr
            key={item.id}
            tabIndex={0}
            data-item-id={item.id}
            className={styles.row}
            onClick={() => onOpen(item)}
            onKeyDown={(event) => {
              if (event.currentTarget === event.target && event.key === "Enter")
                onOpen(item);
            }}
          >
            <td>
              <span className={styles.nameCell}>
                <KindChip kind={item.kind} small />
                <span className={styles.ell}>{item.name}</span>
                <Findings item={item} />
              </span>
            </td>
            <td>{kinds[item.kind].label}</td>
            <td className={styles.mono}>
              <span className={styles.ell}>{sourceText(item)}</span>
            </td>
            <td>
              <span className={styles.rowTags}>
                <TagChips tags={item.tags} />
              </span>
            </td>
            <td className={styles.date}>{importedDate(item)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
