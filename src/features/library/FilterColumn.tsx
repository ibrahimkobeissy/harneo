import { Check, PanelLeftClose } from "lucide-react";
import type { ReactNode } from "react";
import type { Facets } from "../../bindings/Facets";
import { tierLabels, tiers } from "../../ui";
import { FilterResizer } from "./FilterResizer";
import { KindChip } from "./KindChip";
import { kinds, tagTint } from "./kinds";
import styles from "./Library.module.css";
import type { LibraryParams } from "./params";

export type FilterGroup = "types" | "tags" | "sources" | "tiers";

function Option({
  on,
  label,
  count,
  lead,
  onToggle,
}: {
  on: boolean;
  label: string;
  count?: number;
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
      {count !== undefined && <span className={styles.fcount}>{count}</span>}
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
  onToggle,
  onClearGroup,
  onTrustMin,
  onHasFindings,
  onClearTrust,
  dock,
}: {
  facets: Facets;
  params: LibraryParams;
  onToggle: (group: FilterGroup, value: string) => void;
  onClearGroup: (group: FilterGroup) => void;
  onTrustMin: (value: number | null) => void;
  onHasFindings: (on: boolean) => void;
  /** Resets the minimum score, the tiers and Has findings together. */
  onClearTrust: () => void;
  /** Set when the column sits inline beside the screen: it can then collapse and resize. */
  dock?: {
    collapsed: boolean;
    width: number;
    onCollapse: () => void;
    onWidth: (width: number) => void;
    onDragging: (dragging: boolean) => void;
  };
}) {
  // Both filters apply, so a selected tier whose best score is under the slider matches nothing.
  const conflict = tiers
    .filter(
      (t) =>
        params.trustMin !== null &&
        params.tiers.includes(t.tier) &&
        t.hi < params.trustMin,
    )
    .map((t) => t.tier);
  return (
    <aside
      className={styles.filters}
      aria-label="Filters"
      data-collapsed={dock?.collapsed || undefined}
      inert={dock?.collapsed}
    >
      <div className={styles.filtersIn}>
        {dock && (
          <div className={styles.fbar}>
            <span className={styles.ftitle}>Filters</span>
            <button
              type="button"
              className={styles.fcollapse}
              aria-label="Hide filters"
              onClick={dock.onCollapse}
            >
              <PanelLeftClose size={16} aria-hidden="true" />
            </button>
          </div>
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
          title="Trust"
          active={
            params.trustMin !== null ||
            params.tiers.length > 0 ||
            params.hasFindings
          }
          onClear={onClearTrust}
        />
        <label className={styles.trustLabel} htmlFor="trust-min">
          Trust ≥
          <span className={styles.trustValue}>{params.trustMin ?? 0}</span>
        </label>
        <input
          id="trust-min"
          type="range"
          className={styles.trustRange}
          min={0}
          max={100}
          step={5}
          value={params.trustMin ?? 0}
          aria-label="Minimum trust score"
          data-conflict={conflict.length > 0 || undefined}
          onChange={(e) => {
            const n = Number(e.target.value);
            onTrustMin(n > 0 ? n : null);
          }}
        />
        {conflict.length > 0 && (
          <p className={styles.trustNote} role="status">
            Trust ≥ {params.trustMin} rules out{" "}
            {conflict.map((t) => tierLabels[t]).join(" and ")}, so{" "}
            {conflict.length === params.tiers.length
              ? "nothing can match."
              : "those boxes add nothing."}
          </p>
        )}
        {facets.tiers.length === 0 && (
          <span className={styles.muted}>Nothing audited yet</span>
        )}
        {facets.tiers.map(({ tier, count }) => (
          <Option
            key={tier}
            on={params.tiers.includes(tier)}
            label={tierLabels[tier]}
            count={count}
            lead={<i className={styles.tierDot} data-tier={tier} />}
            onToggle={() => onToggle("tiers", tier)}
          />
        ))}
        <Option
          on={params.hasFindings}
          label="Has findings"
          count={facets.withFindings}
          onToggle={() => onHasFindings(!params.hasFindings)}
        />
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
      {dock && (
        <FilterResizer
          width={dock.width}
          onWidth={dock.onWidth}
          onDragging={dock.onDragging}
        />
      )}
    </aside>
  );
}
