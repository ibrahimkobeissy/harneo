import { Hand } from "lucide-react";
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
  const where = branch ? `${label} @ ${branch}` : label;
  return item.fromPlugin ? `${where} · from plugin ${item.fromPlugin}` : where;
}

/** "Update 3.4.1" (warn); just "Update" when upstream has no version to name. */
export function UpdateTag({ item }: { item: LibraryItem }) {
  const { update } = item;
  if (update === null) return null;
  return (
    <Badge tone="warn">
      <span
        title={
          update.localEdits
            ? "Changed at its source; you also edited the library copy"
            : "Changed at its source since you imported it"
        }
      >
        {update.toVersion ? `Update ${update.toVersion}` : "Update"}
      </span>
    </Badge>
  );
}

/** On a part of a plugin that must now stay whole (ADR-006): the reason is the tooltip. */
export function WholeTag({ item }: { item: LibraryItem }) {
  if (item.mustStayWhole === undefined) return null;
  return (
    <Badge tone="warn">
      <span
        title={`Taken apart from ${item.fromPlugin ?? "its plugin"}, which only works whole: ${item.mustStayWhole}`}
      >
        Stays whole
      </span>
    </Badge>
  );
}

/**
 * A quiet marker for a skill that is manual only (never auto-invoked): an outlined chip, not a
 * status badge, so it sits beside the tags and leaves the card foot's one badge alone.
 */
export function ManualChip({
  item,
  short = false,
}: {
  item: LibraryItem;
  /** The list's trailing slot: the hand alone, the words kept for screen readers. */
  short?: boolean;
}) {
  if (!item.manualOnly) return null;
  return (
    <span
      className={styles.manualChip}
      title="Never auto-invoked: runs only when you call it"
    >
      <Hand size={11} aria-hidden="true" />
      {short ? <span className={styles.sr}>Manual only</span> : "Manual only"}
    </span>
  );
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
 * Scan errors and warnings (lint, not security) as one badge: red while there is an error, amber
 * otherwise. "scan" in the label keeps it apart from audit findings. `short` (the list's trailing
 * slot) shows a bare count and keeps the breakdown in the tooltip and for screen readers.
 */
export function ScanBadge({
  item,
  short = false,
}: {
  item: LibraryItem;
  short?: boolean;
}) {
  const { errorCount: e, warningCount: w } = item;
  if (e === 0 && w === 0) return null;
  const parts = [
    e > 0 && `${e} scan ${e === 1 ? "error" : "errors"}`,
    w > 0 && `${w} ${e > 0 ? "" : "scan "}${w === 1 ? "warning" : "warnings"}`,
  ].filter(Boolean);
  const full = parts.join(" · ");
  return (
    <Badge tone={e > 0 ? "bad" : "warn"}>
      {short ? (
        <>
          <span title={full} aria-hidden="true">
            {e + w} scan
          </span>
          <span className={styles.sr}>{full}</span>
        </>
      ) : (
        full
      )}
    </Badge>
  );
}

function FindingsBadge({ item }: { item: LibraryItem }) {
  return (
    <Badge tone={item.tier === "dangerous" ? "bad" : "warn"}>
      {item.findingCount} {item.findingCount === 1 ? "finding" : "findings"}
    </Badge>
  );
}

/**
 * The trust meter of a library item. Risky and Dangerous items get the tier word (`md`) and an
 * "N findings" badge so they cannot be missed; everything else keeps the small bar.
 */
export function ItemTrust({
  item,
  badge = true,
}: {
  item: LibraryItem;
  /** False where the foot shows one badge of its own (the card). */
  badge?: boolean;
}) {
  const loud = item.tier === "risky" || item.tier === "dangerous";
  return (
    <span className={styles.trustCell}>
      <TrustMeter
        score={item.trustScore}
        tier={item.tier}
        size={loud ? "md" : "sm"}
      />
      {badge && loud && item.findingCount > 0 && <FindingsBadge item={item} />}
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

/** "today", "2d ago", "3w ago", "4mo ago", "1y ago": short enough for a narrow list column. */
export function importedAge(item: LibraryItem, now: Date = new Date()): string {
  const days = Math.floor(
    (now.getTime() / 1000 - Number(item.importedAt)) / 86_400,
  );
  if (days < 1) return "today";
  if (days < 14) return `${days}d ago`;
  if (days < 60) return `${Math.floor(days / 7)}w ago`;
  if (days < 365) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

/** "Bundle of N": the artifact plus what it needs, transitively. Nothing when it needs nothing. */
export function BundleBadge({ item }: { item: LibraryItem }) {
  if (item.bundleSize === 0) return null;
  return (
    <Badge tone="neutral">
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

/**
 * The one badge of a card foot, most urgent first: an update, the audit's findings, scan
 * errors and warnings, outdated, then the bundle size. The rest lives in the drawer.
 */
export function CardBadge({
  item,
  status,
}: {
  item: LibraryItem;
  status: Status;
}) {
  if (item.update) return <UpdateTag item={item} />;
  if (item.mustStayWhole !== undefined) return <WholeTag item={item} />;
  if (
    (item.tier === "risky" || item.tier === "dangerous") &&
    item.findingCount > 0
  )
    return <FindingsBadge item={item} />;
  if (status === "findings") return <StatusBadge status={status} />;
  if (item.errorCount > 0 || item.warningCount > 0)
    return <ScanBadge item={item} />;
  if (status === "outdated") return <StatusBadge status={status} />;
  return <BundleBadge item={item} />;
}
