import {
  Check,
  ChevronDown,
  ChevronRight,
  Minus,
  PanelLeftClose,
  Plus,
} from "lucide-react";
import type { ReactNode } from "react";
import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { Facets } from "../bindings/Facets";
import { tierLabels, tiers } from "./audit";
import { FilterResizer } from "./FilterResizer";
import {
  type FilterGroup,
  type FilterParams,
  type Invocation,
  invocationLabels,
  invocations,
  statusLabels,
} from "./filterParams";
import styles from "./Filters.module.css";
import { KindChip } from "./KindChip";
import { kinds, tagTint } from "./kinds";
import { decodeBool, usePersisted } from "./usePersisted";

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

const sections = [
  "type",
  "trust",
  "status",
  "invocation",
  "tag",
  "source",
] as const;
type Section = (typeof sections)[number];

/** Open/closed per group, remembered; Tag starts collapsed. */
function useSections() {
  const [type, setType] = usePersisted(
    "harneo.filters.open.type",
    true,
    decodeBool,
  );
  const [trust, setTrust] = usePersisted(
    "harneo.filters.open.trust",
    true,
    decodeBool,
  );
  const [status, setStatus] = usePersisted(
    "harneo.filters.open.status",
    true,
    decodeBool,
  );
  const [invocation, setInvocation] = usePersisted(
    "harneo.filters.open.invocation",
    true,
    decodeBool,
  );
  const [tag, setTag] = usePersisted(
    "harneo.filters.open.tag",
    false,
    decodeBool,
  );
  const [source, setSource] = usePersisted(
    "harneo.filters.open.source",
    true,
    decodeBool,
  );
  const open: Record<Section, boolean> = {
    type,
    trust,
    status,
    invocation,
    tag,
    source,
  };
  const setters: Record<Section, (next: boolean) => void> = {
    type: setType,
    trust: setTrust,
    status: setStatus,
    invocation: setInvocation,
    tag: setTag,
    source: setSource,
  };
  return {
    open,
    toggle: (k: Section) => setters[k](!open[k]),
    setAll: (next: boolean) => sections.forEach((k) => setters[k](next)),
  };
}

function GroupHead({
  title,
  active,
  open,
  onToggle,
  onClear,
}: {
  title: string;
  /** How many filters are active in this group. */
  active: number;
  open: boolean;
  onToggle: () => void;
  onClear: () => void;
}) {
  const Chevron = open ? ChevronDown : ChevronRight;
  return (
    <h3 className={styles.fhead}>
      <button
        type="button"
        className={styles.fdisclose}
        aria-expanded={open}
        onClick={onToggle}
      >
        <Chevron size={14} aria-hidden="true" />
        {title}
        {!open && active > 0 && (
          <span className={styles.factive} aria-label={`${active} active`}>
            {active}
          </span>
        )}
      </button>
      {active > 0 && (
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
  indexed = true,
  onlyKinds,
  dock,
}: {
  facets: Facets;
  params: FilterParams;
  onToggle: (group: FilterGroup, value: string) => void;
  onClearGroup: (group: FilterGroup) => void;
  onTrustMin: (value: number | null) => void;
  onHasFindings: (on: boolean) => void;
  /** Resets the minimum score, the tiers and Has findings together. */
  onClearTrust: () => void;
  /** False until an index run has completed: Unused is not offered before then. */
  indexed?: boolean;
  /** The kinds Type offers; all of them when unset. */
  onlyKinds?: readonly ArtifactKind[];
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
  const sec = useSections();
  const tagOn = (name: string) =>
    params.tags.some((x) => x.toLowerCase() === name.toLowerCase());
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
        <div className={styles.fbar}>
          <span className={styles.ftitle}>Filters</span>
          <span className={styles.fbtns}>
            <button
              type="button"
              className={styles.fcollapse}
              aria-label="Collapse all filter groups"
              onClick={() => sec.setAll(false)}
            >
              <Minus size={16} aria-hidden="true" />
            </button>
            <button
              type="button"
              className={styles.fcollapse}
              aria-label="Expand all filter groups"
              onClick={() => sec.setAll(true)}
            >
              <Plus size={16} aria-hidden="true" />
            </button>
            {dock && (
              <button
                type="button"
                className={styles.fcollapse}
                aria-label="Hide filters"
                onClick={dock.onCollapse}
              >
                <PanelLeftClose size={16} aria-hidden="true" />
              </button>
            )}
          </span>
        </div>
        <GroupHead
          title="Type"
          active={params.types.length}
          open={sec.open.type}
          onToggle={() => sec.toggle("type")}
          onClear={() => onClearGroup("types")}
        />
        {sec.open.type && (
          <>
            {facets.kinds
              .filter(({ kind }) => !onlyKinds || onlyKinds.includes(kind))
              .map(({ kind, count }) => (
                <Option
                  key={kind}
                  on={params.types.includes(kind)}
                  label={kinds[kind].plural}
                  count={count}
                  lead={<KindChip kind={kind} small />}
                  onToggle={() => onToggle("types", kind)}
                />
              ))}
          </>
        )}
        <GroupHead
          title="Trust"
          active={
            (params.trustMin !== null ? 1 : 0) +
            params.tiers.length +
            (params.hasFindings ? 1 : 0)
          }
          open={sec.open.trust}
          onToggle={() => sec.toggle("trust")}
          onClear={onClearTrust}
        />
        {sec.open.trust && (
          <>
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
          </>
        )}
        <GroupHead
          title="Status"
          active={params.statuses.length}
          open={sec.open.status}
          onToggle={() => sec.toggle("status")}
          onClear={() => onClearGroup("statuses")}
        />
        {sec.open.status && (
          <>
            {facets.statuses
              .filter((s) => indexed || s.status !== "unused")
              .map(({ status, count }) => (
                <Option
                  key={status}
                  on={params.statuses.includes(status)}
                  label={statusLabels[status]}
                  count={count}
                  onToggle={() => onToggle("statuses", status)}
                />
              ))}
          </>
        )}
        <GroupHead
          title="Invocation"
          active={params.invocation.length}
          open={sec.open.invocation}
          onToggle={() => sec.toggle("invocation")}
          onClear={() => onClearGroup("invocation")}
        />
        {sec.open.invocation && (
          <>
            {invocations.map((value: Invocation) => (
              <Option
                key={value}
                on={params.invocation.includes(value)}
                label={invocationLabels[value]}
                count={value === "auto" ? facets.auto : facets.manualOnly}
                onToggle={() => onToggle("invocation", value)}
              />
            ))}
          </>
        )}
        <GroupHead
          title="Tag"
          active={params.tags.length}
          open={sec.open.tag}
          onToggle={() => sec.toggle("tag")}
          onClear={() => onClearGroup("tags")}
        />
        {sec.open.tag && (
          <>
            {facets.tags.length === 0 && (
              <span className={styles.muted}>No tags yet</span>
            )}
            {facets.tags
              .filter((t) => t.count > 0 || tagOn(t.name))
              .map((t) => (
                <Option
                  key={t.name}
                  on={tagOn(t.name)}
                  label={t.name}
                  count={t.count}
                  lead={
                    <i className={styles.tagDot} data-tint={tagTint(t.color)} />
                  }
                  onToggle={() => onToggle("tags", t.name)}
                />
              ))}
          </>
        )}
        <GroupHead
          title="Source"
          active={params.sources.length}
          open={sec.open.source}
          onToggle={() => sec.toggle("source")}
          onClear={() => onClearGroup("sources")}
        />
        {sec.open.source && (
          <>
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
          </>
        )}
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
