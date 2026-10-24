import { ListFilter, Trash2, X } from "lucide-react";
import type { Facets } from "../../bindings/Facets";
import type { TagColor } from "../../bindings/TagColor";
import type { TagWithCount } from "../../bindings/TagWithCount";
import { TagAll } from "../import/TagAll";
import { Button, tierLabels } from "../../ui";
import { kinds } from "./kinds";
import styles from "./Library.module.css";
import type { FilterGroup } from "./FilterColumn";
import { type LibraryParams, statusLabels } from "./params";

const MAX_TAG_CHARS = 40;

/**
 * What the results are: one removable chip per active filter, "N of M artifacts", Clear all
 * and, in the library, the quick actions on every filtered artifact: Save as smart collection, Tag… and Delete….
 */
export function ResultsToolbar({
  params,
  facets,
  total,
  unfilteredTotal,
  filtered,
  onRemove,
  onClearText,
  onClearAll,
  onClearTrustMin,
  onClearHasFindings,
  onSaveCollection,
  onTag,
  allTags,
  onDelete,
}: {
  params: LibraryParams;
  facets: Facets;
  total: number;
  unfilteredTotal: number;
  filtered: boolean;
  onRemove: (group: FilterGroup, value: string) => void;
  onClearText: () => void;
  onClearAll: () => void;
  onClearTrustMin: () => void;
  onClearHasFindings: () => void;
  onSaveCollection?: () => void;
  /** Tag every filtered artifact; shown only while a filter is active. */
  onTag?: (name: string, color: TagColor | null) => void;
  /** Every library tag, for the Tag… picker. */
  allTags?: readonly TagWithCount[];
  /** Delete every filtered artifact; shown only while a filter is active. */
  onDelete?: () => void;
}) {
  const chips: { key: string; label: string; remove: () => void }[] = [
    ...params.types.map((t) => ({
      key: `type:${t}`,
      label: kinds[t].plural,
      remove: () => onRemove("types", t),
    })),
    ...params.tags.map((t) => ({
      key: `tag:${t}`,
      label: t,
      remove: () => onRemove("tags", t),
    })),
    ...params.sources.map((id) => {
      const source = facets.sources.find((s) => s.source.id === id)?.source;
      return {
        key: `source:${id}`,
        label: source
          ? source.branch
            ? `${source.label} @ ${source.branch}`
            : source.label
          : id,
        remove: () => onRemove("sources", id),
      };
    }),
    ...(params.trustMin === null
      ? []
      : [
          {
            key: "trust",
            label: `Trust ≥ ${params.trustMin}`,
            remove: onClearTrustMin,
          },
        ]),
    ...params.tiers.map((t) => ({
      key: `tier:${t}`,
      label: tierLabels[t],
      remove: () => onRemove("tiers", t),
    })),
    ...params.statuses.map((t) => ({
      key: `status:${t}`,
      label: statusLabels[t],
      remove: () => onRemove("statuses", t),
    })),
    ...(params.hasFindings
      ? [
          {
            key: "findings",
            label: "Has findings",
            remove: onClearHasFindings,
          },
        ]
      : []),
    ...(params.text.trim() === ""
      ? []
      : [
          {
            key: "text",
            label: `“${params.text.trim()}”`,
            remove: onClearText,
          },
        ]),
  ];
  const noun = unfilteredTotal === 1 ? "artifact" : "artifacts";
  return (
    <>
      {chips.map((c) => (
        <span key={c.key} className={styles.fchip}>
          <span className={styles.fchipLabel} title={c.label}>
            {c.label}
          </span>
          <button
            type="button"
            className={styles.fchipX}
            aria-label={`Remove filter ${c.label}`}
            onClick={c.remove}
          >
            <X size={12} aria-hidden="true" />
          </button>
        </span>
      ))}
      <span className={styles.count} aria-live="polite">
        {total}
        {total !== unfilteredTotal ? ` of ${unfilteredTotal}` : ""} {noun}
      </span>
      {filtered && (
        <Button small variant="ghost" onClick={onClearAll}>
          Clear all
        </Button>
      )}
      {filtered && onSaveCollection && (
        <Button small variant="ghost" onClick={onSaveCollection}>
          <ListFilter size={14} aria-hidden="true" />
          Save as smart collection
        </Button>
      )}
      {filtered && onTag && (
        <TagAll
          existing={allTags ?? []}
          skip={new Set()}
          maxChars={MAX_TAG_CHARS}
          label="Tag…"
          onPick={onTag}
          variant="toolbar"
        />
      )}
      {filtered && onDelete && (
        <Button small variant="ghost" onClick={onDelete}>
          <Trash2 size={14} aria-hidden="true" />
          Delete…
        </Button>
      )}
    </>
  );
}
