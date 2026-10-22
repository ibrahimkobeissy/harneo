import type { LibraryItem } from "../../bindings/LibraryItem";
import { Findings, importedDate, sourceText, TagChips } from "./ItemMeta";
import { KindChip } from "./KindChip";
import { kinds } from "./kinds";
import styles from "./Library.module.css";

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
          <span className={styles.name}>{item.name}</span>
          <span className={styles.kindLabel}>{kinds[item.kind].label}</span>
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
        <Findings item={item} />
        <span className={styles.date}>{importedDate(item)}</span>
      </span>
    </button>
  );
}
