import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Status } from "../../bindings/Status";
import { Badge, type BadgeTone, Tag, TrustMeter } from "../../ui";
import { tagTint } from "./kinds";
import styles from "./Library.module.css";
import { statusLabels } from "./params";
import ustyles from "./Usage.module.css";
import { plural, relativeDay } from "./usage";

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

/**
 * Scan error and warning counts (lint, not security): "scan" in the label keeps them apart from
 * audit findings. Renders nothing when both are zero.
 */
export function Findings({ item }: { item: LibraryItem }) {
  if (item.errorCount === 0 && item.warningCount === 0) return null;
  return (
    <span className={styles.findings}>
      {item.errorCount > 0 && (
        <Badge tone="bad">
          {item.errorCount} scan {item.errorCount === 1 ? "error" : "errors"}
        </Badge>
      )}
      {item.warningCount > 0 && (
        <Badge tone="warn">
          {item.warningCount} scan{" "}
          {item.warningCount === 1 ? "warning" : "warnings"}
        </Badge>
      )}
    </span>
  );
}

/**
 * The trust meter of a library item. Risky and Dangerous items get the tier word (`md`) and an
 * "N findings" badge so they cannot be missed; everything else keeps the small bar.
 */
export function ItemTrust({ item }: { item: LibraryItem }) {
  const loud = item.tier === "risky" || item.tier === "dangerous";
  return (
    <span className={styles.trustCell}>
      <TrustMeter
        score={item.trustScore}
        tier={item.tier}
        size={loud ? "md" : "sm"}
      />
      {loud && item.findingCount > 0 && (
        <Badge tone={item.tier === "dangerous" ? "bad" : "warn"}>
          {item.findingCount} {item.findingCount === 1 ? "finding" : "findings"}
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

/** "Bundle of N": the artifact plus what it needs, transitively. Nothing when it needs nothing. */
export function BundleBadge({ item }: { item: LibraryItem }) {
  if (item.bundleSize === 0) return null;
  return (
    <Badge tone="info">
      <span title="Installed together, always">{`Bundle of ${item.bundleSize}`}</span>
    </Badge>
  );
}

const statusBadge: Record<Status, { tone: BadgeTone; label: string }> = {
  findings: { tone: "bad", label: statusLabels.findings },
  outdated: { tone: "warn", label: statusLabels.outdated },
  unused: { tone: "neutral", label: statusLabels.unused },
  notUsedYet: { tone: "info", label: statusLabels.notUsedYet },
  current: { tone: "ok", label: statusLabels.current },
};

/** The status as a badge (list column, and the card footer when it needs attention). */
export function StatusBadge({ status }: { status: Status }) {
  const { tone, label } = statusBadge[status];
  return <Badge tone={tone}>{label}</Badge>;
}

/**
 * The usage line of a card footer: sessions, "unused · last X", "not used yet", or "Not tracked"
 * for kinds the logs do not record. Nothing while the first index is still running.
 */
export function UsageFoot({
  item,
  status,
  indexed,
}: {
  item: LibraryItem;
  status: Status;
  indexed: boolean;
}) {
  if (item.usage === null) {
    return (
      <span
        className={ustyles.foot}
        title="Claude's logs do not record when this kind is used"
      >
        Not tracked
      </span>
    );
  }
  if (!indexed) return null;
  const { sessions, lastUsed } = item.usage;
  let text: string;
  if (status === "unused")
    text = lastUsed ? `unused · last ${relativeDay(lastUsed)}` : "unused";
  else if (status === "notUsedYet") text = "not used yet";
  else text = plural(sessions, "session");
  return (
    <span
      className={ustyles.foot}
      title={
        lastUsed
          ? `${plural(sessions, "session")} in the last 8 weeks; last used ${lastUsed}`
          : `${plural(sessions, "session")} in the last 8 weeks`
      }
    >
      {text}
    </span>
  );
}
