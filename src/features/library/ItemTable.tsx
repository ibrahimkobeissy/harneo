import { ArrowDown, ArrowUp } from "lucide-react";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { SortKey } from "../../bindings/SortKey";
import {
  BundleBadge,
  Findings,
  importedDate,
  ItemTrust,
  sourceText,
  StatusBadge,
  TagChips,
} from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import styles from "./Library.module.css";
import { sortKeys, sortLabels } from "./params";
import ustyles from "./Usage.module.css";
import { shownStatus } from "./usage";

export function ItemTable({
  items,
  sort,
  descending,
  onSort,
  onOpen,
  indexed = true,
}: {
  items: LibraryItem[];
  sort: SortKey;
  descending: boolean;
  onSort: (key: SortKey) => void;
  onOpen: (item: LibraryItem) => void;
  indexed?: boolean;
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
                  title={
                    key === "sessions"
                      ? "Sessions in the last 8 weeks"
                      : undefined
                  }
                  onClick={() => onSort(key)}
                >
                  {sortLabels[key]}
                  {active && <Arrow size={12} aria-hidden="true" />}
                </button>
              </th>
            );
          })}
          <th scope="col">Status</th>
          <th scope="col">Bundle</th>
        </tr>
      </thead>
      <tbody>
        {items.map((item) => (
          <tr
            key={item.id}
            className={styles.row}
            data-tier={item.tier ?? undefined}
            data-unused={shownStatus(item, indexed) === "unused" || undefined}
            onClick={(event) => {
              // The name button handles its own click; the rest of the row delegates to it.
              if (!(
                event.target instanceof Element &&
                event.target.closest("button")
              ))
                onOpen(item);
            }}
          >
            <td>
              <span className={styles.nameCell}>
                <KindChip kind={item.kind} small />
                <button
                  type="button"
                  className={styles.rowName}
                  data-item-id={item.id}
                  title={item.name}
                  onClick={() => onOpen(item)}
                >
                  {item.name}
                </button>
                <Findings item={item} />
              </span>
            </td>
            <td>{kinds[item.kind].label}</td>
            <td className={styles.mono}>
              <span className={styles.ell} title={sourceText(item)}>
                {sourceText(item)}
              </span>
            </td>
            <td>
              <span className={styles.rowTags}>
                <TagChips tags={item.tags} />
              </span>
            </td>
            <td>
              <ItemTrust item={item} />
            </td>
            <td className={styles.date}>{importedDate(item)}</td>
            <td className={ustyles.sessionsCell}>
              {item.usage === null ? (
                <span title="Not tracked: the logs do not record this kind">
                  —
                </span>
              ) : indexed ? (
                item.usage.sessions
              ) : null}
            </td>
            <td>
              <StatusBadge status={shownStatus(item, indexed)} />
            </td>
            <td>
              <BundleBadge item={item} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
