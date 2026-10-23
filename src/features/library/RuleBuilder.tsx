import { Check, CircleAlert, ListFilter } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Facets } from "../../bindings/Facets";
import type { Filter } from "../../bindings/Filter";
import type { TagWithCount } from "../../bindings/TagWithCount";
import { libraryList, tagsList } from "../../lib/ipc";
import { kinds } from "./kinds";
import styles from "./Collections.module.css";

export const emptyRule: Filter = {
  types: [],
  tags: [],
  sources: [],
  text: null,
  trustMin: null,
  tiers: [],
  hasFindings: false,
};

/** True when the rule has no condition at all (a smart collection needs one). */
export function ruleIsEmpty(rule: Filter): boolean {
  return (
    rule.types.length + rule.tags.length + rule.sources.length === 0 &&
    (rule.text ?? "").trim() === ""
  );
}

/** The rule as stored: blank text becomes `null`. */
export function cleanRule(rule: Filter): Filter {
  const text = (rule.text ?? "").trim();
  return { ...rule, text: text === "" ? null : text };
}

const everyKind = Object.keys(kinds) as ArtifactKind[];

/** The library's facets with no filter, for the source names. `null` until they arrive. */
export function useLibraryFacets(enabled = true): Facets | null {
  const [facets, setFacets] = useState<Facets | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let current = true;
    libraryList({
      filter: emptyRule,
      sort: "name",
      descending: false,
      offset: 0,
      limit: 1,
    }).then(
      (page) => {
        if (current) setFacets(page.facets);
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, [enabled]);
  return facets;
}

function toggled<T>(list: T[], value: T): T[] {
  return list.includes(value)
    ? list.filter((v) => v !== value)
    : [...list, value];
}

function Chip({
  on,
  label,
  onToggle,
}: {
  on: boolean;
  label: string;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className={on ? `${styles.rchip} ${styles.rchipOn}` : styles.rchip}
      aria-pressed={on}
      onClick={onToggle}
    >
      {on && <Check size={12} strokeWidth={2.5} aria-hidden="true" />}
      {label}
    </button>
  );
}

/** How many artifacts the rule matches right now, or `null` while it is being counted. */
function useMatchCount(rule: Filter): number | null {
  const key = JSON.stringify(cleanRule(rule));
  const [counted, setCounted] = useState<{ key: string; n: number } | null>(
    null,
  );
  useEffect(() => {
    const parsed = JSON.parse(key) as Filter;
    if (ruleIsEmpty(parsed)) return;
    let current = true;
    // A short wait so typing in the text field does not ask for every keystroke.
    const timer = setTimeout(() => {
      libraryList({
        filter: parsed,
        sort: "name",
        descending: false,
        offset: 0,
        limit: 1,
      }).then(
        (page) => {
          if (current) setCounted({ key, n: page.total });
        },
        () => {},
      );
    }, 150);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [key]);
  return counted?.key === key ? counted.n : null;
}

/**
 * The smart collection's rule as chips: Type, Tag and Source (any within a row, all rows must
 * match) and a text field, with a live count of what the rule matches now.
 */
export function RuleBuilder({
  value,
  onChange,
}: {
  value: Filter;
  onChange: (rule: Filter) => void;
}) {
  const [tags, setTags] = useState<TagWithCount[]>([]);
  const facets = useLibraryFacets();
  const textId = useId();
  const count = useMatchCount(value);
  const empty = ruleIsEmpty(value);

  useEffect(() => {
    let current = true;
    tagsList().then(
      (list) => {
        if (current) setTags(list);
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, []);

  // A rule can name a tag or source the lists no longer show; keep it visible to switch off.
  const tagNames = [
    ...tags.map((t) => t.name),
    ...value.tags.filter((name) => !tags.some((t) => t.name === name)),
  ];
  const sources = facets?.sources.map((s) => s.source) ?? [];
  const sourceChips = [
    ...sources.map((s) => ({
      id: s.id,
      label: s.branch ? `${s.label} @ ${s.branch}` : s.label,
    })),
    ...value.sources
      .filter((id) => !sources.some((s) => s.id === id))
      .map((id) => ({ id, label: id })),
  ];

  return (
    <div className={styles.rulebox}>
      <div className={styles.rrow}>
        <span id={`${textId}-type`}>Type</span>
        <div role="group" aria-labelledby={`${textId}-type`}>
          {everyKind.map((kind) => (
            <Chip
              key={kind}
              on={value.types.includes(kind)}
              label={kinds[kind].plural}
              onToggle={() =>
                onChange({ ...value, types: toggled(value.types, kind) })
              }
            />
          ))}
        </div>
      </div>
      <div className={styles.rrow}>
        <span id={`${textId}-tag`}>Tag</span>
        <div role="group" aria-labelledby={`${textId}-tag`}>
          {tagNames.length === 0 && (
            <span className={styles.muted}>No tags yet</span>
          )}
          {tagNames.map((name) => (
            <Chip
              key={name}
              on={value.tags.includes(name)}
              label={name}
              onToggle={() =>
                onChange({ ...value, tags: toggled(value.tags, name) })
              }
            />
          ))}
        </div>
      </div>
      <div className={styles.rrow}>
        <span id={`${textId}-source`}>Source</span>
        <div role="group" aria-labelledby={`${textId}-source`}>
          {sourceChips.length === 0 && (
            <span className={styles.muted}>No sources yet</span>
          )}
          {sourceChips.map((s) => (
            <Chip
              key={s.id}
              on={value.sources.includes(s.id)}
              label={s.label}
              onToggle={() =>
                onChange({ ...value, sources: toggled(value.sources, s.id) })
              }
            />
          ))}
        </div>
      </div>
      <div className={styles.rrow}>
        <label htmlFor={textId}>Text</label>
        <input
          id={textId}
          className={styles.input}
          placeholder="Name, description or tag contains…"
          autoComplete="off"
          value={value.text ?? ""}
          onChange={(e) => onChange({ ...value, text: e.target.value })}
        />
      </div>
      <p
        className={empty || count === 0 ? styles.hintWarn : styles.hint}
        aria-live="polite"
      >
        {empty || count === 0 ? (
          <CircleAlert size={14} aria-hidden="true" />
        ) : (
          <ListFilter size={14} aria-hidden="true" />
        )}
        {empty
          ? "Pick at least one condition. Conditions in different rows must all match; within a row, any."
          : count === null
            ? "Counting what matches…"
            : count === 0
              ? "Nothing matches this rule yet. It will fill as you import."
              : `Matches ${count} ${count === 1 ? "artifact" : "artifacts"} now. New imports that match join automatically.`}
      </p>
    </div>
  );
}
