import { Check, Plus, Search } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { LibraryItem } from "../../bindings/LibraryItem";
import { libraryList } from "../../lib/ipc";
import { Button, kinds } from "../../ui";
import styles from "./Builder.module.css";

type Segment = "all" | ArtifactKind;

const segments: { value: Segment; label: string }[] = [
  { value: "all", label: "All" },
  { value: "skill", label: "Skills" },
  { value: "agent", label: "Subagents" },
  { value: "command", label: "Commands" },
  { value: "rule", label: "Rules" },
  { value: "plugin", label: "Plugins" },
  { value: "outputStyle", label: "Styles" },
  { value: "claudeMd", label: "CLAUDE.md" },
];

/** Settings and MCP have their own panels in the cart. */
const pickable: ArtifactKind[] = [
  "skill",
  "agent",
  "command",
  "rule",
  "plugin",
  "outputStyle",
  "claudeMd",
  "workflow",
];

const LIMIT = 40;
const DEBOUNCE_MS = 200;

type Rows =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; items: LibraryItem[] };

/** The library, most used first, to pick from. */
export function Picker({
  picked,
  onAdd,
  onRemove,
}: {
  picked: Set<string>;
  onAdd: (item: LibraryItem) => void;
  onRemove: (id: string) => void;
}) {
  const searchId = useId();
  const [text, setText] = useState("");
  const [segment, setSegment] = useState<Segment>("all");
  const [rows, setRows] = useState<Rows>({ status: "loading" });

  useEffect(() => {
    let stale = false;
    const timer = setTimeout(() => {
      libraryList({
        filter: {
          types: segment === "all" ? pickable : [segment],
          tags: [],
          sources: [],
          text: text.trim() === "" ? null : text.trim(),
          trustMin: null,
          tiers: [],
          hasFindings: false,
          outdated: false,
          statuses: [],
        },
        sort: "sessions",
        descending: true,
        offset: 0,
        limit: LIMIT,
      }).then(
        (page) => {
          if (!stale) setRows({ status: "ready", items: page.items });
        },
        (err: unknown) => {
          if (!stale) setRows({ status: "error", message: String(err) });
        },
      );
    }, DEBOUNCE_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [text, segment]);

  return (
    <section className={styles.picker} aria-labelledby="picker-title">
      <h2 id="picker-title" className={styles.pickerTitle}>
        Pick from your library
      </h2>
      <p className={styles.quiet}>Adding an artifact adds its whole bundle.</p>
      <div className={styles.toolbar}>
        <div className={styles.search}>
          <label className={styles.sr} htmlFor={searchId}>
            Search library
          </label>
          <Search size={14} aria-hidden="true" />
          <input
            id={searchId}
            className={styles.searchInput}
            placeholder="Search library"
            autoComplete="off"
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        </div>
        <div className={styles.seg} role="group" aria-label="Type">
          {segments.map((s) => (
            <button
              key={s.value}
              type="button"
              className={segment === s.value ? styles.on : ""}
              aria-pressed={segment === s.value}
              onClick={() => setSegment(s.value)}
            >
              {s.label}
            </button>
          ))}
        </div>
      </div>
      {rows.status === "error" && (
        <p role="alert" className={styles.errorText}>
          The library did not load: {rows.message}
        </p>
      )}
      {rows.status === "ready" && rows.items.length === 0 && (
        <p className={styles.quiet}>No matches.</p>
      )}
      {rows.status === "ready" && rows.items.length > 0 && (
        <ul className={styles.pgrid} aria-label="Library">
          {rows.items.map((item) => (
            <PickRow
              key={item.id}
              item={item}
              added={picked.has(item.id)}
              onAdd={onAdd}
              onRemove={onRemove}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function PickRow({
  item,
  added,
  onAdd,
  onRemove,
}: {
  item: LibraryItem;
  added: boolean;
  onAdd: (item: LibraryItem) => void;
  onRemove: (id: string) => void;
}) {
  const meta = kinds[item.kind];
  const Icon = meta.icon;
  const line = [
    item.bundleSize > 0 ? `bundle of ${item.bundleSize + 1}` : null,
    item.source.label,
    item.trustScore === null ? "not audited" : `trust ${item.trustScore}`,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <li className={`${styles.prow} ${added ? styles.in : ""}`}>
      <span className={styles.kindIcon} title={meta.label}>
        <Icon size={14} aria-hidden="true" />
      </span>
      <div className={styles.rowText}>
        <div className={styles.rowName}>{item.name}</div>
        <div className={styles.srcline}>{line}</div>
      </div>
      {added ? (
        <Button
          small
          aria-label={`Remove ${item.name}`}
          onClick={() => onRemove(item.id)}
        >
          <Check size={13} aria-hidden="true" />
          Added
        </Button>
      ) : (
        <Button
          small
          aria-label={`Add ${item.name}`}
          onClick={() => onAdd(item)}
        >
          <Plus size={13} aria-hidden="true" />
          Add
        </Button>
      )}
    </li>
  );
}
