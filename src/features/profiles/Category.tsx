import { ChevronDown, Workflow } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { CategoryId } from "../../bindings/CategoryId";
import type { Template } from "../../bindings/Template";
import { Button } from "../../ui";
import { categoryHint, customPreview, labelOf } from "./category";
import styles from "./Category.module.css";
import { Dialog, NameField } from "./Dialog";
import { ShapeThumb } from "./ShapeThumb";

/** The choice a new profile is created with. */
export interface NewChoice {
  name: string;
  category: CategoryId;
  /** Custom's description; `null` for every other category. */
  note: string | null;
}

/** The stages of a category's template, or Custom's promise, as the preview line. */
function Stages({ template }: { template: Template | undefined }) {
  const names =
    template && template.id !== "custom"
      ? template.stages.map((s) => s.name)
      : [customPreview];
  return (
    <div className={styles.stages}>
      <div className={styles.cap}>
        Template stages · the empty Harnmap you start from
      </div>
      <ol className={styles.stageList} aria-label="Template stages">
        {names.map((n, i) => (
          <li key={`${i}-${n}`}>
            <span className={styles.stage}>{n}</span>
            {i < names.length - 1 && <span aria-hidden="true">→</span>}
          </li>
        ))}
      </ol>
    </div>
  );
}

/** New profile: a name, a category from the radio cards, Custom's description, the stages. */
export function NewProfileDialog({
  templates,
  initialName,
  onCreate,
  onCancel,
}: {
  templates: Template[];
  initialName: string;
  onCreate: (choice: NewChoice) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initialName);
  const [category, setCategory] = useState<CategoryId>("software");
  const [note, setNote] = useState("");
  const noteId = useId();
  const custom = category === "custom";
  const ids = templates.map((t) => t.id);

  return (
    <Dialog
      title="New profile"
      submit="Create profile"
      busy={false}
      wide
      onSubmit={() =>
        onCreate({
          name,
          category,
          note: custom && note.trim() !== "" ? note.trim() : null,
        })
      }
      onClose={onCancel}
    >
      <NameField value={name} onChange={setName} />
      <div>
        <div className={styles.askRow}>
          <h3 className={styles.ask}>What kind of work is it for?</h3>
          <span className={styles.askHint}>
            Sets the shape of its Harnmap. You can change it later.
          </span>
        </div>
        <div role="radiogroup" aria-label="Category" className={styles.cards}>
          {ids.map((id) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={id === category}
              className={styles.cat}
              onClick={() => setCategory(id)}
            >
              <ShapeThumb id={id} />
              <b>{labelOf(templates, id)}</b>
              <span>{categoryHint[id]}</span>
            </button>
          ))}
        </div>
      </div>
      {custom && (
        <div className={styles.field}>
          <label htmlFor={noteId}>Describe the work in a few lines</label>
          <textarea
            id={noteId}
            className={styles.ta}
            value={note}
            placeholder="e.g. I run a weekly newsletter on climate policy: collect news, pick 5 stories, write, have a friend review, send on Fridays."
            onChange={(e) => setNote(e.target.value)}
          />
          <span className={styles.askHint}>
            Claude picks one of the 5 shapes and names the stages when you first
            draw the map. Until then the map shows one open stage.
          </span>
        </div>
      )}
      <Stages template={templates.find((t) => t.id === category)} />
    </Dialog>
  );
}

/** The builder's category control: a menu of the eight categories; the current one is marked. */
export function CategoryMenu({
  templates,
  current,
  onPick,
}: {
  templates: Template[];
  current: CategoryId;
  onPick: (id: CategoryId) => void;
}) {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    function onDown(ev: MouseEvent) {
      if (wrap.current && !wrap.current.contains(ev.target as Node))
        setOpen(false);
    }
    function onKeyDown(ev: KeyboardEvent) {
      if (ev.key !== "Escape") return;
      ev.preventDefault();
      ev.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open]);

  return (
    <div className={styles.menuRow} ref={wrap}>
      <span className={styles.cap}>Category</span>
      <Button
        small
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Workflow size={13} aria-hidden="true" />
        {labelOf(templates, current)}
        <ChevronDown size={13} aria-hidden="true" />
      </Button>
      {open && (
        <div className={styles.menu} role="menu" aria-label="Category">
          {templates.map((t) => (
            <button
              key={t.id}
              type="button"
              role="menuitem"
              className={styles.menuItem}
              onClick={() => {
                setOpen(false);
                onPick(t.id);
              }}
            >
              {t.label}
              {t.id === current && <span className={styles.mark}>current</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** The inline confirm for a category change on a saved profile. */
export function CategoryConfirm({
  profile,
  currentLabel,
  targetLabel,
  custom,
  busy,
  onKeep,
  onSwitch,
}: {
  profile: string;
  currentLabel: string;
  targetLabel: string;
  /** The target is Custom: ask for its description too. */
  custom: boolean;
  busy: boolean;
  onKeep: () => void;
  onSwitch: (note: string | null) => void;
}) {
  const [note, setNote] = useState("");
  const noteId = useId();
  return (
    <div
      className={styles.confirm}
      role="alertdialog"
      aria-label="Change category"
    >
      <div className={styles.confirmText}>
        <b>
          Switch {profile} to {targetLabel}?
        </b>{" "}
        Nothing is sent now. The map is marked out of date, and you re-draw it
        on the {targetLabel} template when you&rsquo;re ready.
        {custom && (
          <div className={styles.field}>
            <label htmlFor={noteId}>Describe the work in a few lines</label>
            <textarea
              id={noteId}
              className={styles.ta}
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </div>
        )}
      </div>
      <Button onClick={onKeep}>Keep {currentLabel}</Button>
      <Button
        variant="primary"
        disabled={busy}
        onClick={() =>
          onSwitch(custom && note.trim() !== "" ? note.trim() : null)
        }
      >
        Switch category
      </Button>
    </div>
  );
}
