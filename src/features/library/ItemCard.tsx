import type { LibraryItem } from "../../bindings/LibraryItem";
import { TrustMeter } from "../../ui";
import { CardCounts, Findings, sourceText, TagChips } from "./ItemMeta";
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
        <TrustMeter score={item.trustScore} tier={item.tier} />
        <span className={styles.tileMeta}>
          <CardCounts item={item} />
          <Findings item={item} />
        </span>
      </span>
    </button>
  );
}
