import { Bookmark, BookmarkPlus, Check, ChevronDown, Plus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useCollections } from "../../app/collections";
import {
  artifactCollections,
  collectionAdd,
  collectionRemove,
} from "../../lib/ipc";
import { Button, announce } from "../../ui";
import styles from "./Collections.module.css";

/**
 * The detail panel's "Add to…" menu: the manual collections, checked when the artifact is in
 * them (a click toggles), and "New collection…". `onChanged` fires after membership changed.
 */
export function AddToMenu({
  artifactId,
  onChanged,
}: {
  artifactId: string;
  onChanged: () => void;
}) {
  const { collections, refresh, openNew } = useCollections();
  const [open, setOpen] = useState(false);
  const [member, setMember] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);
  const wrap = useRef<HTMLDivElement>(null);
  const manual = collections.filter((c) => c.kind === "manual");

  useEffect(() => {
    if (!open) return;
    let current = true;
    artifactCollections(artifactId).then(
      (ids) => {
        if (current) setMember(Array.isArray(ids) ? ids : []);
      },
      (err: unknown) => {
        if (current) setError(String(err));
      },
    );
    function onDown(event: MouseEvent) {
      if (wrap.current && !wrap.current.contains(event.target as Node))
        setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      // Captured so the panel behind does not also close on this Esc.
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      current = false;
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open, artifactId]);

  async function toggle(id: bigint) {
    const n = Number(id);
    setError(null);
    try {
      const name = collections.find((c) => c.id === id)?.name ?? "collection";
      if (member.includes(n)) {
        await collectionRemove(id, artifactId);
        setMember(member.filter((m) => m !== n));
        announce(`Removed from ${name}`);
      } else {
        await collectionAdd(id, artifactId);
        setMember([...member, n]);
        announce(`Added to ${name}`);
      }
      refresh();
      onChanged();
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className={styles.anchor} ref={wrap}>
      <Button
        small
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Plus size={14} aria-hidden="true" />
        Add to…
        <ChevronDown size={14} aria-hidden="true" />
      </Button>
      {open && (
        <div className={styles.menu} role="menu" aria-label="Add to">
          {manual.map((c) => {
            const on = member.includes(Number(c.id));
            return (
              <button
                key={String(c.id)}
                type="button"
                role="menuitemcheckbox"
                aria-checked={on}
                className={styles.menuItem}
                onClick={() => void toggle(c.id)}
              >
                <Bookmark size={14} aria-hidden="true" />
                <span className={styles.menuName}>{c.name}</span>
                {on && <Check size={14} aria-label="added" />}
              </button>
            );
          })}
          {manual.length === 0 && (
            <p className={styles.menuEmpty}>No manual collections yet.</p>
          )}
          <button
            type="button"
            role="menuitem"
            className={styles.menuItem}
            onClick={() => {
              setOpen(false);
              openNew();
            }}
          >
            <BookmarkPlus size={14} aria-hidden="true" />
            New collection…
          </button>
          {error && (
            <p role="alert" className={styles.error}>
              {error}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
