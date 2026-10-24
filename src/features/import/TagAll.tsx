import { Plus, Tag as TagIcon } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { TagColor } from "../../bindings/TagColor";
import type { TagWithCount } from "../../bindings/TagWithCount";
import { Button, Tag } from "../../ui";
import styles from "./Import.module.css";
import { palette } from "./tagColor";

type Option =
  { kind: "existing"; tag: TagWithCount } | { kind: "create"; name: string };

/**
 * "Tag all": a menu to pick an existing library tag or create one with a palette color, and
 * hand it back to apply to every artifact being imported. Mirrors the detail panel's Add tag.
 */
export function TagAll({
  existing,
  skip,
  maxChars,
  onPick,
  label = "Tag all",
  variant = "dashed",
}: {
  existing: readonly TagWithCount[];
  /** Lowercased tag names already on every row; they are not offered again. */
  skip: ReadonlySet<string>;
  maxChars: number;
  /** `color` is set only for a new name; an existing tag keeps its own. */
  onPick: (name: string, color: TagColor | null) => void;
  /** The button's text; the default is the import step's "Tag all". */
  label?: string;
  /** `toolbar` is a ghost toolbar button (Library); the default is the import step's dashed one. */
  variant?: "dashed" | "toolbar";
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [color, setColor] = useState<TagColor>("cyan");
  const wrap = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const listId = useId();

  useEffect(() => {
    if (!open) return;
    function onDown(event: MouseEvent) {
      if (wrap.current && !wrap.current.contains(event.target as Node))
        setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const q = query.trim().toLowerCase();
  const options: Option[] = existing
    .filter((t) => !skip.has(t.name.toLowerCase()))
    .filter((t) => t.name.toLowerCase().includes(q))
    .map((tag) => ({ kind: "existing", tag }));
  const exact = existing.some((t) => t.name.toLowerCase() === q);
  if (q && !exact && !skip.has(q)) {
    options.push({ kind: "create", name: query.trim() });
  }
  const at = Math.min(active, Math.max(options.length - 1, 0));
  const creating = options.some((o) => o.kind === "create");

  function close() {
    setOpen(false);
    trigger.current?.focus();
  }

  function toggle() {
    setOpen(!open);
    setQuery("");
    setActive(0);
    requestAnimationFrame(() => input.current?.focus());
  }

  function pick(option: Option) {
    if (option.kind === "existing") onPick(option.tag.name, null);
    else onPick(option.name, color);
    close();
  }

  function onKeyDown(e: React.KeyboardEvent) {
    const pressed = e.key;
    if (pressed === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (pressed === "ArrowDown" && options.length > 0) {
      e.preventDefault();
      setActive((at + 1) % options.length);
    } else if (pressed === "ArrowUp" && options.length > 0) {
      e.preventDefault();
      setActive((at - 1 + options.length) % options.length);
    } else if (pressed === "Enter" && e.target === input.current) {
      e.preventDefault();
      const option = options[at];
      if (option) pick(option);
    }
  }

  return (
    <div className={styles.tagAllAnchor} ref={wrap}>
      {variant === "toolbar" ? (
        <Button
          small
          variant="ghost"
          ref={trigger}
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={toggle}
        >
          <TagIcon size={14} aria-hidden="true" />
          {label}
        </Button>
      ) : (
        <button
          type="button"
          ref={trigger}
          className={styles.addTag}
          aria-haspopup="listbox"
          aria-expanded={open}
          onClick={toggle}
        >
          <Plus size={12} aria-hidden="true" />
          {label}
        </button>
      )}
      {open && (
        <div
          className={styles.tagMenu}
          role="presentation"
          onKeyDown={onKeyDown}
        >
          <input
            ref={input}
            className={styles.tagSearch}
            role="combobox"
            aria-label={`Find or create a tag: ${label}`}
            aria-expanded="true"
            aria-controls={listId}
            aria-activedescendant={options[at] ? `${listId}-${at}` : undefined}
            placeholder="Find or create a tag…"
            maxLength={maxChars}
            autoComplete="off"
            spellCheck={false}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
            }}
          />
          <div id={listId} role="listbox" aria-label="Tags">
            {options.map((option, i) => (
              <div
                key={option.kind === "existing" ? `t${option.tag.id}` : "new"}
                id={`${listId}-${i}`}
                role="option"
                tabIndex={-1}
                aria-selected={i === at}
                className={styles.tagOption}
                data-active={i === at}
                onMouseEnter={() => setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => pick(option)}
                onKeyDown={() => {}}
              >
                {option.kind === "existing" ? (
                  <>
                    <Tag tint={option.tag.color} dot>
                      {option.tag.name}
                    </Tag>
                    <span className={styles.tagCount}>{option.tag.count}</span>
                  </>
                ) : (
                  <span className={styles.tagCreate}>
                    <Plus size={13} aria-hidden="true" />
                    <span>{`Create “${option.name}”`}</span>
                  </span>
                )}
              </div>
            ))}
          </div>
          {options.length === 0 && (
            <p className={styles.tagEmpty}>
              {existing.length === 0
                ? "No tags yet. Type a name to create one."
                : "No other tags."}
            </p>
          )}
          {creating && (
            <div
              className={styles.tagColors}
              role="radiogroup"
              aria-label="Color of the new tag"
            >
              {palette.map((c) => (
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
              <span className={styles.tagColorName}>{color}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
