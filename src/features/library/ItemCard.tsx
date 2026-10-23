import type { LibraryItem } from "../../bindings/LibraryItem";
import {
  CardBadge,
  CardCounts,
  ItemTrust,
  sourceText,
  TagChips,
  UsageFoot,
} from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import styles from "./Library.module.css";
import { shownStatus } from "./usage";

/** Single-instance types (Settings, CLAUDE.md) are named after their type: say it once. */
function sameText(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase();
}

export function ItemCard({
  item,
  onOpen,
  indexed = true,
}: {
  item: LibraryItem;
  onOpen: (item: LibraryItem) => void;
  /** False until an index run has completed: unused is not shown before then. */
  indexed?: boolean;
}) {
  const status = shownStatus(item, indexed);
  return (
    <button
      type="button"
      className={styles.tile}
      data-item-id={item.id}
      data-tier={item.tier ?? undefined}
      data-unused={status === "unused" || undefined}
      onClick={() => onOpen(item)}
    >
      <span className={styles.tileHead}>
        <KindChip kind={item.kind} />
        <span className={styles.tileTitle}>
          <span className={styles.name} title={item.name}>
            {item.name}
          </span>
          {!sameText(item.name, kinds[item.kind].label) && (
            <span className={styles.kindLabel}>{kinds[item.kind].label}</span>
          )}
        </span>
      </span>
      <span className={styles.desc}>{item.description ?? ""}</span>
      <span className={styles.tileTags}>
        <TagChips tags={item.tags} />
      </span>
      <span className={styles.tileFoot}>
        <span className={styles.srcline} title={sourceText(item)}>
          <span className={styles.srcText}>{sourceText(item)}</span>
        </span>
        <ItemTrust item={item} badge={false} />
        <span className={styles.tileMeta}>
          <CardCounts item={item} />
          <UsageFoot item={item} status={status} indexed={indexed} />
          <CardBadge item={item} status={status} />
        </span>
      </span>
    </button>
  );
}
