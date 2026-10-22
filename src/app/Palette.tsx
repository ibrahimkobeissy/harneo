import { Keyboard, Search, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { useNavigate } from "react-router";
import { kinds } from "../features/library/kinds";
import { libraryList } from "../lib/ipc";
import { Kbd } from "../ui";
import { Modal } from "./Modal";
import styles from "./Palette.module.css";
import { actions, places, settingsDestination } from "./routes";

interface Entry {
  id: string;
  label: string;
  group: "Places" | "Actions" | "Help" | "Artifacts";
  icon: LucideIcon;
  /** Where Enter goes; defaults to `id`. */
  to?: string;
  /** Quiet text after the label. */
  hint?: string;
}

const ARTIFACT_LIMIT = 8;
const DEBOUNCE_MS = 120;

/** Library artifacts whose name matches `text`, for the palette's Artifacts group. */
function useArtifactEntries(text: string): Entry[] {
  const [found, setFound] = useState<Entry[]>([]);
  useEffect(() => {
    if (text === "") return;
    let stale = false;
    const timer = setTimeout(() => {
      libraryList({
        filter: { text, types: [], tags: [], sources: [] },
        sort: "name",
        descending: false,
        offset: 0,
        limit: ARTIFACT_LIMIT,
      })
        .then((page) => {
          if (stale) return;
          const rows = page.items.map((it) => ({
            id: `artifact-${it.id}`,
            label: it.name,
            group: "Artifacts" as const,
            icon: kinds[it.kind].icon,
            to: `/library?open=${encodeURIComponent(it.id)}`,
            hint: `${kinds[it.kind].label} · ${it.source.label}`,
          }));
          setFound(rows);
        })
        // Quiet: a failed search just shows no artifacts.
        .catch(() => {
          if (!stale) setFound([]);
        });
    }, DEBOUNCE_MS);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [text]);
  // Keep the previous rows while the next search runs, so the list does not flash.
  return text === "" ? [] : found;
}

const entries: readonly Entry[] = [
  ...[...places, settingsDestination].map((d) => ({
    id: d.path,
    label: d.label,
    group: "Places" as const,
    icon: d.icon,
  })),
  ...actions.map((d) => ({
    id: d.path,
    label: d.label,
    group: "Actions" as const,
    icon: d.icon,
  })),
  {
    id: "shortcuts",
    label: "Keyboard shortcuts",
    group: "Help",
    icon: Keyboard,
  },
];
const groups = ["Places", "Actions", "Help", "Artifacts"] as const;
const optionId = (e: Entry) => `palette-${e.id.replace(/\W/g, "") || "x"}`;

interface PaletteProps {
  onClose: () => void;
  onShortcuts: () => void;
}

export function Palette({ onClose, onShortcuts }: PaletteProps) {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const q = query.trim().toLowerCase();
  const artifactRows = useArtifactEntries(q);
  const matches = useMemo(
    () => [
      ...entries.filter((e) => e.label.toLowerCase().includes(q)),
      ...artifactRows,
    ],
    [q, artifactRows],
  );
  const current = Math.min(active, Math.max(matches.length - 1, 0));
  const activeEntry = matches[current];

  // Keep the highlighted row visible as the list scrolls.
  useEffect(() => {
    if (!activeEntry) return;
    document
      .getElementById(optionId(activeEntry))
      ?.scrollIntoView?.({ block: "nearest" });
  }, [activeEntry]);

  function run(entry: Entry | undefined) {
    if (!entry) return;
    if (entry.id === "shortcuts") return onShortcuts();
    onClose();
    void navigate(entry.to ?? entry.id);
  }

  function onKeyDown(ev: KeyboardEvent<HTMLInputElement>) {
    const n = matches.length;
    if (ev.key === "ArrowDown" && n) {
      ev.preventDefault();
      setActive((current + 1) % n);
    } else if (ev.key === "ArrowUp" && n) {
      ev.preventDefault();
      setActive((current - 1 + n) % n);
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      run(activeEntry);
    }
  }

  return (
    <Modal label="Search and commands" variant="palette" onClose={onClose}>
      <div className={styles.field}>
        <Search size={16} aria-hidden="true" />
        <input
          type="text"
          role="combobox"
          aria-label="Search places and actions"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={
            activeEntry ? optionId(activeEntry) : undefined
          }
          placeholder="Search places and actions"
          autoComplete="off"
          autoFocus
          className={styles.input}
          value={query}
          onChange={(ev) => {
            setQuery(ev.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      <div
        id="palette-list"
        role="listbox"
        aria-label="Results"
        className={styles.results}
      >
        {groups.map((g) => {
          const rows = matches.filter((m) => m.group === g);
          if (!rows.length) return null;
          return (
            <div key={g} role="group" aria-labelledby={`palette-group-${g}`}>
              <div
                id={`palette-group-${g}`}
                role="presentation"
                className={styles.grp}
              >
                {g}
              </div>
              {rows.map((m) => {
                const i = matches.indexOf(m);
                return (
                  <div
                    key={m.id}
                    id={optionId(m)}
                    role="option"
                    aria-selected={i === current}
                    className={
                      i === current
                        ? `${styles.item} ${styles.act}`
                        : styles.item
                    }
                    onMouseMove={() => i !== current && setActive(i)}
                    onClick={() => run(m)}
                  >
                    <m.icon size={16} aria-hidden="true" />
                    {m.label}
                    {m.hint && <span className={styles.hint}>{m.hint}</span>}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
      {matches.length === 0 && (
        <p className={styles.none} role="status">
          No matches for “{query.trim()}”.
        </p>
      )}
      <div className={styles.foot}>
        <span>
          <Kbd>↑</Kbd> <Kbd>↓</Kbd> move
        </span>
        <span>
          <Kbd>Enter</Kbd> open
        </span>
        <span>
          <Kbd>Esc</Kbd> close
        </span>
      </div>
    </Modal>
  );
}
