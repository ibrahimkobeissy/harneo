import { Plus, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { LinkOrigin } from "../../bindings/LinkOrigin";
import type { Links } from "../../bindings/Links";
import type { LinkView } from "../../bindings/LinkView";
import { libraryList, linkAdd, linkRemove, linksOf } from "../../lib/ipc";
import { announce } from "../../ui";
import styles from "./BundleSection.module.css";
import { kinds } from "./kinds";

const originLabels: Record<LinkOrigin, string> = {
  reference: "Reference",
  ai: "Claude",
  manual: "Manual",
};

type Direction = "needs" | "neededBy";

/** Library artifacts whose name matches `text`, debounced; failures show an empty list. */
function useCandidates(text: string, active: boolean): LibraryItem[] {
  const [found, setFound] = useState<LibraryItem[]>([]);
  useEffect(() => {
    if (!active) return;
    let stale = false;
    const timer = setTimeout(() => {
      libraryList({
        filter: {
          text: text.trim() === "" ? null : text.trim(),
          types: [],
          tags: [],
          sources: [],
          trustMin: null,
          tiers: [],
          hasFindings: false,
          outdated: false,
          statuses: [],
        },
        sort: "name",
        descending: false,
        offset: 0,
        limit: 30,
      }).then(
        (page) => {
          if (!stale) setFound(page.items);
        },
        () => {
          if (!stale) setFound([]);
        },
      );
    }, 150);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [text, active]);
  return found;
}

function LinkRow({
  link,
  direction,
  onOpen,
  onRemove,
}: {
  link: LinkView;
  direction: Direction;
  onOpen: (id: string) => void;
  onRemove: (link: LinkView) => void;
}) {
  return (
    <li className={styles.row}>
      <button
        type="button"
        className={styles.open}
        title={link.reason === "" ? undefined : link.reason}
        onClick={() => onOpen(link.id)}
      >
        <span className={styles.name}>{link.name}</span>
        <span className={styles.kind}>{kinds[link.kind].label}</span>
      </button>
      <span className={`${styles.origin} ${styles[link.origin]}`}>
        {originLabels[link.origin]}
      </span>
      {link.origin === "reference" ? (
        <span className={styles.slot} />
      ) : (
        <button
          type="button"
          className={styles.remove}
          aria-label={
            direction === "needs"
              ? `Remove link: needs ${link.name}`
              : `Remove link: needed by ${link.name}`
          }
          onClick={() => onRemove(link)}
        >
          <X size={13} aria-hidden="true" />
        </button>
      )}
      {link.reason !== "" && (
        <span className={styles.reason}>{link.reason}</span>
      )}
    </li>
  );
}

function List({
  title,
  empty,
  links,
  direction,
  onOpen,
  onRemove,
}: {
  title: string;
  empty: string;
  links: LinkView[];
  direction: Direction;
  onOpen: (id: string) => void;
  onRemove: (link: LinkView, direction: Direction) => void;
}) {
  return (
    <div>
      <h4 className={styles.sub}>
        {title} ({links.length})
      </h4>
      {links.length === 0 ? (
        <p className={styles.muted}>{empty}</p>
      ) : (
        <ul className={styles.list} aria-label={title}>
          {links.map((l) => (
            <LinkRow
              key={l.id}
              link={l}
              direction={direction}
              onOpen={onOpen}
              onRemove={(link) => onRemove(link, direction)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * The detail panel's Bundle section: what the artifact needs and what needs it, installed
 * together, always. Clicking a link opens that artifact (the panel keeps a Back trail). Claude
 * and manual links can be removed; reference links come from the artifact's own files. Mount it
 * with `key={artifactId}`.
 */
export function BundleSection({
  artifactId,
  initial,
  onOpen,
  onChanged,
}: {
  artifactId: string;
  initial: Links;
  onOpen: (id: string) => void;
  onChanged: () => void;
}) {
  const [links, setLinks] = useState<Links>(initial);
  const [picking, setPicking] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const listId = useId();
  const found = useCandidates(query, picking);

  // Leave out what it needs already and what needs it, so no A-needs-B-needs-A by hand.
  const taken = new Set([
    artifactId,
    ...links.needs.map((l) => l.id),
    ...links.neededBy.map((l) => l.id),
  ]);
  const options = found.filter((i) => !taken.has(i.id));
  const at = Math.min(active, Math.max(options.length - 1, 0));
  const hasReference = [...links.needs, ...links.neededBy].some(
    (l) => l.origin === "reference",
  );

  const alone = links.needs.length === 0 && links.neededBy.length === 0;

  async function change(action: () => Promise<void>, message: string) {
    setError(null);
    try {
      await action();
      setLinks(await linksOf(artifactId));
      onChanged();
      announce(message);
      return true;
    } catch (err) {
      setError(String(err));
      return false;
    }
  }

  function remove(link: LinkView, direction: Direction) {
    const [from, to] =
      direction === "needs" ? [artifactId, link.id] : [link.id, artifactId];
    void change(
      () => linkRemove(from, to),
      direction === "needs"
        ? `Removed the link to ${link.name}`
        : `Removed the link from ${link.name}`,
    );
  }

  async function pick(target: LibraryItem) {
    const done = await change(
      () => linkAdd(artifactId, target.id, "manual", ""),
      `Now needs ${target.name}`,
    );
    if (done) closePicker();
  }

  function closePicker() {
    setPicking(false);
    setQuery("");
    setActive(0);
    requestAnimationFrame(() => trigger.current?.focus());
  }

  function onKeyDown(event: React.KeyboardEvent) {
    const pressed = event.key;
    if (pressed === "Escape") {
      // The panel's own Escape handler would close the whole panel.
      event.preventDefault();
      event.nativeEvent.stopImmediatePropagation();
      closePicker();
    } else if (pressed === "ArrowDown" && options.length > 0) {
      event.preventDefault();
      setActive((at + 1) % options.length);
    } else if (pressed === "ArrowUp" && options.length > 0) {
      event.preventDefault();
      setActive((at - 1 + options.length) % options.length);
    } else if (pressed === "Enter") {
      event.preventDefault();
      const option = options[at];
      if (option) void pick(option);
    }
  }

  return (
    <section className={styles.sec} aria-labelledby="d-bundle">
      <h3 id="d-bundle">
        Bundle <span className={styles.hint}>installed together, always</span>
      </h3>
      {alone ? (
        <p className={styles.muted}>Needs nothing, nothing needs it.</p>
      ) : (
        <>
          <List
            title="Needs"
            empty="Needs nothing else."
            links={links.needs}
            direction="needs"
            onOpen={onOpen}
            onRemove={remove}
          />
          <List
            title="Needed by"
            empty="Nothing needs it."
            links={links.neededBy}
            direction="neededBy"
            onOpen={onOpen}
            onRemove={remove}
          />
        </>
      )}
      {hasReference && (
        <p className={styles.note}>
          Reference links come from the artifact&rsquo;s own files, so they
          cannot be removed here.
        </p>
      )}
      {picking ? (
        <div
          className={styles.picker}
          role="presentation"
          onKeyDown={onKeyDown}
        >
          <input
            className={styles.search}
            role="combobox"
            aria-label="Find an artifact to link"
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={options[at] ? `${listId}-${at}` : undefined}
            placeholder="Find an artifact it needs…"
            autoComplete="off"
            ref={input}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
          />
          <div id={listId} role="listbox" aria-label="Artifacts">
            {options.map((o, i) => (
              <div
                key={o.id}
                id={`${listId}-${i}`}
                role="option"
                tabIndex={-1}
                aria-selected={i === at}
                className={styles.option}
                data-active={i === at}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => void pick(o)}
                onKeyDown={() => {}}
              >
                <span className={styles.name}>{o.name}</span>
                <span className={styles.kind}>{kinds[o.kind].label}</span>
              </div>
            ))}
          </div>
          {options.length === 0 && (
            <p className={styles.muted}>No other artifact matches.</p>
          )}
        </div>
      ) : (
        <button
          type="button"
          ref={trigger}
          className={styles.add}
          onClick={() => {
            setError(null);
            setPicking(true);
            // The input mounts with the picker.
            requestAnimationFrame(() => input.current?.focus());
          }}
        >
          <Plus size={12} aria-hidden="true" />
          Add a link…
        </button>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </section>
  );
}
