import type { LibraryItem } from "../../bindings/LibraryItem";
import {
  BundleBadge,
  CardCounts,
  Findings,
  ItemTrust,
  sourceText,
  TagChips,
} from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import styles from "./Library.module.css";

/** Single-instance types (Settings, CLAUDE.md) are named after their type: say it once. */
function sameText(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase();
}

export function ItemCard({
  item,
  onOpen,
}: {
  item: LibraryItem;
  onOpen: (item: LibraryItem) => void;
}) {
  return (
    <button
      type="button"
      className={styles.tile}
      data-item-id={item.id}
      data-tier={item.tier ?? undefined}
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
          {sourceText(item)}
        </span>
        <ItemTrust item={item} />
        <span className={styles.tileMeta}>
          <CardCounts item={item} />
          <BundleBadge item={item} />
          <Findings item={item} />
        </span>
      </span>
    </button>
  );
}
