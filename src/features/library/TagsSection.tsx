import { Plus, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { Tag as TagRecord } from "../../bindings/Tag";
import type { TagColor } from "../../bindings/TagColor";
import type { TagWithCount } from "../../bindings/TagWithCount";
import {
  artifactTagAdd,
  artifactTagRemove,
  artifactTags,
  tagCreate,
  tagsList,
} from "../../lib/ipc";
import { Tag } from "../../ui";
import { tagTint } from "./kinds";
import styles from "./TagsSection.module.css";

/** The tag palette, in the order the picker shows it (`TagColor`). */
export const tagColors: readonly TagColor[] = [
  "cyan",
  "sky",
  "indigo",
  "violet",
  "fuchsia",
  "stone",
  "slate",
];

type Option =
  { kind: "existing"; tag: TagWithCount } | { kind: "create"; name: string };

/** The options for what is typed: tags not yet on the artifact, then "Create" for a new name. */
function optionsFor(
  query: string,
  all: TagWithCount[],
  mine: TagRecord[],
): { options: Option[]; note: string | null } {
  const q = query.trim().toLowerCase();
  const onArtifact = new Set(mine.map((t) => t.id));
  const options: Option[] = all
    .filter((t) => !onArtifact.has(t.id) && t.name.toLowerCase().includes(q))
    .map((tag) => ({ kind: "existing", tag }));
  const exact = all.find((t) => t.name.toLowerCase() === q);
  if (q && !exact) options.push({ kind: "create", name: query.trim() });
  const note =
    exact && onArtifact.has(exact.id) ? "This artifact already has it." : null;
  return { options, note };
}

/**
 * The detail panel's Tags section: the artifact's tags as removable chips, and an "Add tag"
 * menu to pick an existing tag or create one with a palette color. `onChanged` tells the
 * library list to reload, so cards, rows and the filter column follow. Mount it with
 * `key={artifactId}` so state starts fresh for each artifact.
 */
export function TagsSection({
  artifactId,
  onChanged,
}: {
  artifactId: string;
  onChanged: () => void;
}) {
  const [mine, setMine] = useState<TagRecord[] | null>(null);
  const [all, setAll] = useState<TagWithCount[]>([]);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [color, setColor] = useState<TagColor>("cyan");
  const [error, setError] = useState<string | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const listId = useId();

  async function refresh() {
    const [tags, counts] = await Promise.all([
      artifactTags(artifactId),
      tagsList(),
    ]);
    setMine(tags);
    setAll(counts);
  }

  useEffect(() => {
    let current = true;
    Promise.all([artifactTags(artifactId), tagsList()]).then(
      ([tags, counts]) => {
        if (!current) return;
        setMine(tags);
        setAll(counts);
      },
      (err: unknown) => {
        if (current) setError(String(err));
      },
    );
    return () => {
      current = false;
    };
  }, [artifactId]);

  useEffect(() => {
    if (!open) return;
    function onDown(event: MouseEvent) {
      if (wrap.current && !wrap.current.contains(event.target as Node))
        setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const { options, note } = optionsFor(query, all, mine ?? []);
  const at = Math.min(active, Math.max(options.length - 1, 0));

  function openMenu() {
    setOpen(true);
    setQuery("");
    setActive(0);
    setError(null);
    tagsList().then(setAll, (err: unknown) => setError(String(err)));
    // The input mounts with the menu.
    requestAnimationFrame(() => input.current?.focus());
  }

  function closeMenu() {
    setOpen(false);
    trigger.current?.focus();
  }

  async function change(action: () => Promise<void>): Promise<boolean> {
    setError(null);
    try {
      await action();
      await refresh();
      onChanged();
      return true;
    } catch (err) {
      setError(String(err));
      return false;
    }
  }

  async function pick(option: Option) {
    const done = await change(async () => {
      if (option.kind === "existing") {
        await artifactTagAdd(artifactId, option.tag.id);
      } else {
        const created = await tagCreate(option.name, color);
        await artifactTagAdd(artifactId, created.id);
      }
    });
    if (done) {
      setQuery("");
      setActive(0);
      input.current?.focus();
    }
  }

  function onKeyDown(event: React.KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu();
    } else if (event.key === "ArrowDown" && options.length > 0) {
      event.preventDefault();
      setActive((at + 1) % options.length);
    } else if (event.key === "ArrowUp" && options.length > 0) {
      event.preventDefault();
      setActive((at - 1 + options.length) % options.length);
    } else if (event.key === "Enter" && event.target === input.current) {
      event.preventDefault();
      const option = options[at];
      if (option) void pick(option);
    }
  }

  const creating = options.some((o) => o.kind === "create");
  return (
    <section className={styles.sec} aria-labelledby="d-tags">
      <h3 id="d-tags">Tags</h3>
      <div className={styles.edit}>
        {(mine ?? []).map((t) => (
          <Tag key={String(t.id)} tint={tagTint(t.color)} dot>
            {t.name}
            <button
              type="button"
              className={styles.remove}
              aria-label={`Remove tag ${t.name}`}
              onClick={() =>
                void change(() => artifactTagRemove(artifactId, t.id))
              }
            >
              <X size={11} aria-hidden="true" />
            </button>
          </Tag>
        ))}
        <div className={styles.anchor} ref={wrap}>
          <button
            type="button"
            ref={trigger}
            className={styles.add}
            aria-haspopup="listbox"
            aria-expanded={open}
            disabled={mine === null}
            onClick={() => (open ? setOpen(false) : openMenu())}
          >
            <Plus size={12} aria-hidden="true" />
            Add tag
          </button>
          {open && (
            <div
              className={styles.menu}
              role="presentation"
              onKeyDown={onKeyDown}
            >
              <input
                ref={input}
                className={styles.search}
                role="combobox"
                aria-label="Find or create a tag"
                aria-expanded="true"
                aria-controls={listId}
                aria-activedescendant={
                  options[at] ? `${listId}-${at}` : undefined
                }
                placeholder="Find or create a tag…"
                autoComplete="off"
                value={query}
                onChange={(e) => {
                  setQuery(e.target.value);
                  setActive(0);
                  setError(null);
                }}
              />
              <div id={listId} role="listbox" aria-label="Tags">
                {options.map((option, i) => (
                  <div
                    key={
                      option.kind === "existing"
                        ? `t${option.tag.id}`
                        : "create"
                    }
                    id={`${listId}-${i}`}
                    role="option"
                    tabIndex={-1}
                    aria-selected={i === at}
                    className={styles.option}
                    data-active={i === at}
                    onMouseEnter={() => setActive(i)}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => void pick(option)}
                    onKeyDown={() => {}}
                  >
                    {option.kind === "existing" ? (
                      <>
                        <Tag tint={tagTint(option.tag.color)} dot>
                          {option.tag.name}
                        </Tag>
                        <span className={styles.count}>{option.tag.count}</span>
                      </>
                    ) : (
                      <>
                        <Plus size={13} aria-hidden="true" />
                        {`Create “${option.name}”`}
                      </>
                    )}
                  </div>
                ))}
              </div>
              {options.length === 0 && (
                <p className={styles.empty}>
                  {note ??
                    (all.length === 0
                      ? "No tags yet. Type a name to create one."
                      : "No other tags.")}
                </p>
              )}
              {creating && (
                <div
                  className={styles.colors}
                  role="radiogroup"
                  aria-label="Color of the new tag"
                >
                  {tagColors.map((c) => (
                    <button
                      key={c}
                      type="button"
                      role="radio"
                      aria-checked={color === c}
                      aria-label={c}
                      title={c}
                      className={styles.swatch}
                      data-tint={c}
                      onMouseDown={(e) => e.preventDefault()}
                      onClick={() => setColor(c)}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </section>
  );
}
