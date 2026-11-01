import { Info } from "lucide-react";
import type { Candidate } from "../../bindings/Candidate";
import type { PluginHold } from "../../bindings/PluginHold";
import { Badge } from "../../ui";
import { groupByKind } from "./groupByKind";
import { kindPlurals } from "./kindLabels";
import {
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
  busy,
  onChange,
}: {
  group: PluginGroup;
  hold: PluginHold | undefined;
  busy: boolean;
  onChange: (mode: PluginMode) => void;
}) {
  const mode: PluginMode = group.parts.length > 0 ? "parts" : "whole";
  const refusal: Record<PluginMode, string | null> = {
    whole: wholeRefusal(group, hold),
    parts: partsRefusal(group, hold),
  };
  const base = `choice-${group.folder || "root"}`;
  return (
    <fieldset className={styles.choice} aria-busy={busy}>
      <legend className={styles.sr}>Import {group.name} as</legend>
      <div className={styles.choices}>
        {OPTIONS.map(([value, label]) => {
          const why = refusal[value];
          const unavailable = why !== null;
          return (
            <label key={value} className={styles.choiceOption}>
              <input
                type="radio"
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
      {mode === "parts" && (
        <p className={`${styles.muted} ${styles.wholeOnly}`}>
          <Info size={12} aria-hidden="true" /> {WHOLE_ONLY}
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
        <input
          type="checkbox"
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

/** The parts of an unpacked plugin, nested under it and grouped by kind. */
export function PluginParts({
  group,
  excluded,
  onToggle,
}: {
  group: PluginGroup;
  excluded: ReadonlySet<string>;
  onToggle: (id: string, on: boolean) => void;
}) {
  return (
    <div className={styles.parts}>
      {groupByKind(group.parts).map(([kind, list]) => (
        <section
          key={kind}
          aria-label={`${kindPlurals[kind]} of ${group.name}, ${list.length}`}
        >
          <h4 className={styles.subhead}>
            {kindPlurals[kind]}{" "}
            <span className={styles.count}>{list.length}</span>
          </h4>
          <ul className={styles.plain}>
            {list.map((c) => (
              <Part
                key={c.id}
                candidate={c}
                ticked={!excluded.has(c.id)}
                onToggle={onToggle}
              />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
