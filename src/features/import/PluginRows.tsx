import { ChevronRight, Info } from "lucide-react";
import { useId, useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { PluginHold } from "../../bindings/PluginHold";
import { Badge, Checkbox, Radio } from "../../ui";
import type { WholeNote } from "../../ui/plugin";
import { groupByKind } from "./groupByKind";
import { kindPlurals } from "./kindLabels";
import {
  lostIfUnpacked,
  needsPlugin,
  partsRefusal,
  type PluginGroup,
  WHOLE_ONLY,
  wholeRefusal,
} from "./plugins";
import styles from "./Import.module.css";

export type PluginMode = "whole" | "parts";

const OPTIONS: [PluginMode, string][] = [
  ["whole", "Whole plugin"],
  ["parts", "Pick parts"],
];

/**
 * The two-way choice of a plugin: the whole plugin or some of its parts. A choice that cannot be
 * made stays focusable and says why, in words next to it, so the reason is not hover only.
 */
export function PluginChoice({
  group,
  hold,
  reason,
  busy,
  onChange,
}: {
  group: PluginGroup;
  hold: PluginHold | undefined;
  /** Why the plugin is proposed whole, when it must be (ADR-004). */
  reason: WholeNote | null;
  busy: boolean;
  onChange: (mode: PluginMode) => void;
}) {
  const mode: PluginMode = group.parts.length > 0 ? "parts" : "whole";
  const refusal: Record<PluginMode, string | null> = {
    whole: wholeRefusal(group, hold),
    parts: partsRefusal(group, hold),
  };
  const base = `choice-${group.folder || "root"}`;
  const lost = lostIfUnpacked(reason);
  return (
    <fieldset className={styles.pluginChoice} aria-busy={busy}>
      <legend className={styles.sr}>Import {group.name} as</legend>
      <div className={styles.pluginChoices}>
        {OPTIONS.map(([value, label]) => {
          const why = refusal[value];
          const unavailable = why !== null;
          return (
            <label key={value} className={styles.choiceOption}>
              <Radio
                name={base}
                checked={mode === value}
                aria-disabled={unavailable || busy}
                aria-describedby={why ? `${base}-${value}` : undefined}
                onChange={() => {
                  if (!unavailable && !busy) onChange(value);
                }}
              />
              <span>{label}</span>
            </label>
          );
        })}
      </div>
      {OPTIONS.map(([value, label]) => {
        const why = refusal[value];
        if (!why) return null;
        return (
          <p key={value} id={`${base}-${value}`} className={styles.reason}>
            {label}: {why}
          </p>
        );
      })}
      {mode === "whole" && reason && (
        <p className={`${styles.muted} ${styles.wholeOnly}`}>
          <Info size={12} aria-hidden="true" /> {reason.text}
        </p>
      )}
      {mode === "parts" && (
        <p className={`${styles.muted} ${styles.wholeOnly}`}>
          <Info size={12} aria-hidden="true" /> {WHOLE_ONLY}
        </p>
      )}
      {mode === "parts" && lost && (
        <p className={`${styles.muted} ${styles.wholeOnly}`}>
          <Info size={12} aria-hidden="true" /> Lost if unpacked: {lost}.
        </p>
      )}
    </fieldset>
  );
}

/** One part: a checkbox, or a disabled one with its reason written out beside it. */
function Part({
  candidate,
  ticked,
  onToggle,
}: {
  candidate: Candidate;
  ticked: boolean;
  onToggle: (id: string, on: boolean) => void;
}) {
  const reason = needsPlugin(candidate);
  const reasonId = `part-${candidate.id}-reason`;
  return (
    <li className={styles.part}>
      <label className={styles.partLabel}>
        <span className={styles.slot} aria-hidden="true" />
        <Checkbox
          checked={reason === null && ticked}
          aria-disabled={reason !== null}
          aria-describedby={reason ? reasonId : undefined}
          onChange={(ev) => {
            if (reason === null) onToggle(candidate.id, ev.target.checked);
          }}
        />
        <span className={styles.grow}>
          <span className={styles.title}>
            <b>{candidate.artifact.name}</b>
            {candidate.alreadyInLibrary && <Badge>Already in library</Badge>}
          </span>
          <span className={`${styles.line} ${styles.block}`}>
            <span className={styles.mono}>{candidate.artifact.path}</span>
          </span>
          {candidate.artifact.description && (
            <span className={`${styles.desc} ${styles.block}`}>
              {candidate.artifact.description}
            </span>
          )}
        </span>
      </label>
      {reason && (
        <p id={reasonId} className={styles.reason}>
          Stays with the plugin: {reason}
        </p>
      )}
    </li>
  );
}

/** A plugin with more parts than this starts folded. */
export const FOLD_OVER = 8;

/** A checkbox for many parts at once: ticked when all are, mixed when some are. */
function AllBox({
  label,
  ticked,
  total,
  onSet,
}: {
  label: string;
  ticked: number;
  total: number;
  onSet: (on: boolean) => void;
}) {
  const some = ticked > 0 && ticked < total;
  return (
    <Checkbox
      aria-label={label}
      aria-checked={some ? "mixed" : ticked === total && total > 0}
      checked={ticked === total && total > 0}
      indeterminate={some}
      disabled={total === 0}
      onChange={() => onSet(ticked < total)}
    />
  );
}

/** A kind's plural title in running text: "skills", "1 command", but "MCP servers" kept as is. */
function kindWord(plural: string, n: number): string {
  const word = n === 1 && plural.endsWith("s") ? plural.slice(0, -1) : plural;
  const second = word.charAt(1);
  return second && second === second.toLowerCase()
    ? word.charAt(0).toLowerCase() + word.slice(1)
    : word;
}

/** What a folded plugin holds, in one line: "7 of 9 ticked · 9 commands, 14 skills". */
export function partsSummary(
  groups: [string, Candidate[]][],
  ticked: number,
  pickable: number,
): string {
  const kinds = groups
    .map(([plural, list]) => `${list.length} ${kindWord(plural, list.length)}`)
    .join(", ");
  return `${ticked} of ${pickable} ticked · ${kinds}`;
}

/**
 * The parts of an unpacked plugin, nested under it and grouped by kind. The list and each kind
 * fold; a plugin with many parts starts folded unless a part needs attention. The boxes on the
 * plugin and on each kind tick or untick every part that can be picked.
 */
export function PluginParts({
  group,
  excluded,
  onSet,
}: {
  group: PluginGroup;
  excluded: ReadonlySet<string>;
  onSet: (ids: string[], on: boolean) => void;
}) {
  const pickable = group.parts.filter((c) => needsPlugin(c) === null);
  const attention = pickable.length < group.parts.length;
  const [open, setOpen] = useState(
    group.parts.length <= FOLD_OVER || attention,
  );
  const [shut, setShut] = useState<ReadonlySet<string>>(new Set());
  const bodyId = useId();
  const byKind = groupByKind(group.parts).map(
    ([kind, list]) => [kindPlurals[kind], list] as [string, Candidate[]],
  );
  const toggleKind = (plural: string) =>
    setShut((prev) => {
      const next = new Set(prev);
      if (next.has(plural)) next.delete(plural);
      else next.add(plural);
      return next;
    });
  const tickedOf = (list: Candidate[]) =>
    list.filter((c) => needsPlugin(c) === null && !excluded.has(c.id)).length;
  const ids = (list: Candidate[]) =>
    list.filter((c) => needsPlugin(c) === null).map((c) => c.id);
  const ticked = tickedOf(pickable);
  return (
    <div className={styles.parts}>
      <div className={styles.partsHead}>
        <button
          type="button"
          className={`${styles.chevron} ${styles.toggle}`}
          aria-hidden="true"
          tabIndex={-1}
          onClick={() => setOpen(!open)}
        >
          <ChevronRight size={14} aria-hidden="true" data-open={open} />
        </button>
        <AllBox
          label={`All parts of ${group.name}`}
          ticked={ticked}
          total={pickable.length}
          onSet={(on) => onSet(ids(pickable), on)}
        />
        <button
          type="button"
          className={styles.toggle}
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => setOpen(!open)}
        >
          <span>{open ? "Parts" : `${group.parts.length} parts`}</span>
        </button>
        <span className={styles.muted} aria-live="polite">
          {partsSummary(byKind, ticked, pickable.length)}
        </span>
      </div>
      {open && (
        <div id={bodyId}>
          {byKind.map(([plural, list]) => {
            const folded = shut.has(plural);
            const kindId = `${bodyId}-${plural}`;
            return (
              <section
                key={plural}
                aria-label={`${plural} of ${group.name}, ${list.length}`}
              >
                <div className={styles.kindHead}>
                  <button
                    type="button"
                    className={`${styles.chevron} ${styles.toggle}`}
                    aria-hidden="true"
                    tabIndex={-1}
                    onClick={() => toggleKind(plural)}
                  >
                    <ChevronRight
                      size={12}
                      aria-hidden="true"
                      data-open={!folded}
                    />
                  </button>
                  <AllBox
                    label={`All ${plural.toLowerCase()} of ${group.name}`}
                    ticked={tickedOf(list)}
                    total={ids(list).length}
                    onSet={(on) => onSet(ids(list), on)}
                  />
                  <button
                    type="button"
                    className={`${styles.toggle} ${styles.subhead}`}
                    aria-expanded={!folded}
                    aria-controls={kindId}
                    onClick={() => toggleKind(plural)}
                  >
                    {plural} <span className={styles.count}>{list.length}</span>
                  </button>
                </div>
                {!folded && (
                  <ul id={kindId} className={styles.plain}>
                    {list.map((c) => (
                      <Part
                        key={c.id}
                        candidate={c}
                        ticked={!excluded.has(c.id)}
                        onToggle={(id, on) => onSet([id], on)}
                      />
                    ))}
                  </ul>
                )}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
