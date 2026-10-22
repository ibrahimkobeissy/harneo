import { Check, ListFilter } from "lucide-react";
import type { ReactNode } from "react";
import type { Facets } from "../../bindings/Facets";
import { Button } from "../../ui";
import { KindChip } from "./KindChip";
import { kinds, tagTint } from "./kinds";
import styles from "./Library.module.css";
import type { LibraryParams } from "./params";

export type FilterGroup = "types" | "tags" | "sources";

function Option({
  on,
  label,
  count,
  lead,
  onToggle,
}: {
  on: boolean;
  label: string;
  count: number;
  lead?: ReactNode;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={on ? `${styles.fopt} ${styles.foptOn}` : styles.fopt}
      aria-pressed={on}
      onClick={onToggle}
    >
      <i className={styles.box}>
        {on && <Check size={11} strokeWidth={3} aria-hidden="true" />}
      </i>
      {lead}
      <span className={styles.flabel}>{label}</span>
      <span className={styles.fcount}>{count}</span>
    </button>
  );
}

function GroupHead({
  title,
  active,
  onClear,
}: {
  title: string;
  active: boolean;
  onClear: () => void;
}) {
  return (
    <h3 className={styles.fhead}>
      {title}
      {active && (
        <button
          type="button"
          className={styles.fclear}
          aria-label={`Clear ${title.toLowerCase()} filter`}
          onClick={onClear}
        >
          Clear
        </button>
      )}
    </h3>
  );
}

export function FilterColumn({
  facets,
  params,
  filtered,
  onToggle,
  onClearGroup,
  onClearAll,
  onSaveCollection,
}: {
  facets: Facets;
  params: LibraryParams;
  filtered: boolean;
  onToggle: (group: FilterGroup, value: string) => void;
  onClearGroup: (group: FilterGroup) => void;
  onClearAll: () => void;
  /** Offered while filters are on (the Library only): save them as a smart collection. */
  onSaveCollection?: () => void;
}) {
  return (
    <aside className={styles.filters} aria-label="Filters">
      <div className={styles.filtersIn}>
        {filtered && (
          <Button small variant="ghost" onClick={onClearAll}>
            Clear filters
          </Button>
        )}
        {filtered && onSaveCollection && (
          <Button small variant="ghost" onClick={onSaveCollection}>
            <ListFilter size={14} aria-hidden="true" />
            Save as smart collection
          </Button>
        )}
        <GroupHead
          title="Type"
          active={params.types.length > 0}
          onClear={() => onClearGroup("types")}
        />
        {facets.kinds.map(({ kind, count }) => (
          <Option
            key={kind}
            on={params.types.includes(kind)}
            label={kinds[kind].plural}
            count={count}
            lead={<KindChip kind={kind} small />}
            onToggle={() => onToggle("types", kind)}
          />
        ))}
        <GroupHead
          title="Tag"
          active={params.tags.length > 0}
          onClear={() => onClearGroup("tags")}
        />
        {facets.tags.length === 0 && (
          <span className={styles.muted}>No tags yet</span>
        )}
        {facets.tags.map((t) => (
          <Option
            key={t.name}
            on={params.tags.some(
              (x) => x.toLowerCase() === t.name.toLowerCase(),
            )}
            label={t.name}
            count={t.count}
            lead={<i className={styles.tagDot} data-tint={tagTint(t.color)} />}
            onToggle={() => onToggle("tags", t.name)}
          />
        ))}
        <GroupHead
          title="Source"
          active={params.sources.length > 0}
          onClear={() => onClearGroup("sources")}
        />
        {facets.sources.map(({ source, count }) => (
          <Option
            key={source.id}
            on={params.sources.includes(source.id)}
            label={
              source.branch
                ? `${source.label} @ ${source.branch}`
                : source.label
            }
            count={count}
            onToggle={() => onToggle("sources", source.id)}
          />
        ))}
      </div>
    </aside>
  );
}
