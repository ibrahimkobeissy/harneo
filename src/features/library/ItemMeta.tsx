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

export function importedDate(item: LibraryItem): string {
  return new Date(Number(item.importedAt) * 1000).toLocaleDateString();
}
