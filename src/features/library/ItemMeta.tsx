import type { LibraryItem } from "../../bindings/LibraryItem";
import { Badge, Tag } from "../../ui";
import { tagTint } from "./kinds";
import styles from "./Library.module.css";

export function sourceText(item: LibraryItem): string {
  const { label, branch } = item.source;
  return branch ? `${label} @ ${branch}` : label;
}

/** At most `max` tag chips, then a "+n" for the rest. */
export function TagChips({
  tags,
  max = 2,
}: {
  tags: LibraryItem["tags"];
  max?: number;
}) {
  return (
    <>
      {tags.slice(0, max).map((t) => (
        <Tag key={t.name} tint={tagTint(t.color)} dot>
          {t.name}
        </Tag>
      ))}
      {tags.length > max && <Tag tint="plain">{`+${tags.length - max}`}</Tag>}
    </>
  );
}

/** Error and warning counts; renders nothing when both are zero. */
export function Findings({ item }: { item: LibraryItem }) {
  if (item.errorCount === 0 && item.warningCount === 0) return null;
  return (
    <span className={styles.findings}>
      {item.errorCount > 0 && (
        <Badge tone="bad">
          {item.errorCount} {item.errorCount === 1 ? "error" : "errors"}
        </Badge>
      )}
      {item.warningCount > 0 && (
        <Badge tone="warn">
          {item.warningCount} {item.warningCount === 1 ? "warning" : "warnings"}
        </Badge>
      )}
    </span>
  );
}

/** File count and resolved/total references, for the card footer. */
export function CardCounts({ item }: { item: LibraryItem }) {
  const files = `${item.fileCount} ${item.fileCount === 1 ? "file" : "files"}`;
  const refs =
    item.referenceCount === 0
      ? null
      : `${item.resolvedReferences}/${item.referenceCount} ${item.referenceCount === 1 ? "ref" : "refs"}`;
  return (
    <span
      className={styles.counts}
      title={
        refs
          ? `${files}; ${item.resolvedReferences} of ${item.referenceCount} references found in the source`
          : files
      }
    >
      {files}
      {refs && ` · ${refs}`}
    </span>
  );
}

const dateFormat = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  year: "numeric",
});

/** "21 Oct 2025": the month is spelled out, so the day and month cannot be swapped. */
export function formatDate(seconds: bigint | number): string {
  return dateFormat.format(new Date(Number(seconds) * 1000));
}

export function importedDate(item: LibraryItem): string {
  return formatDate(item.importedAt);
}
