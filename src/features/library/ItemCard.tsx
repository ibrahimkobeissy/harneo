import type { ArtifactFit } from "../../bindings/ArtifactFit";
import type { LibraryItem } from "../../bindings/LibraryItem";
import { FitBadges } from "./FitBadges";
import {
  CardBadge,
  CardCounts,
  ItemTrust,
  ManualChip,
  sourceText,
  TagChips,
  UsageFoot,
} from "./ItemMeta";
import { KindChip } from "../../ui";
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
  fits = [],
}: {
  item: LibraryItem;
  onOpen: (item: LibraryItem) => void;
  /** Saved Harnfit verdicts of this artifact, one per profile. */
  fits?: ArtifactFit[];
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
        <ManualChip item={item} />
        <TagChips tags={item.tags} />
      </span>
      <FitBadges fits={fits} />
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
