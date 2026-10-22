import { X } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import styles from "./Library.module.css";

const focusable = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

/**
 * The filter column as an overlay sheet from the left edge: focus moves in, is trapped on
 * Tab, Esc or a click on the scrim closes it, and focus returns to `returnFocus` (the
 * Filters button) on close.
 */
export function FilterSheet({
  onClose,
  returnFocus,
  children,
}: {
  onClose: () => void;
  returnFocus: { current: HTMLElement | null };
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(focusable)?.focus();
    const back = returnFocus;
    return () => back.current?.focus();
  }, [returnFocus]);

  function onKeyDown(ev: KeyboardEvent<HTMLDivElement>) {
    if (ev.key === "Escape") {
      ev.preventDefault();
      ev.stopPropagation();
      onClose();
      return;
    }
    if (ev.key !== "Tab") return;
    const node = ref.current;
    if (!node) return;
    const items = Array.from(node.querySelectorAll<HTMLElement>(focusable));
    const first = items.at(0);
    const last = items.at(-1);
    if (!first || !last) return;
    const active = document.activeElement;
    if (ev.shiftKey && (active === first || active === node)) {
      ev.preventDefault();
      last.focus();
    } else if (!ev.shiftKey && active === last) {
      ev.preventDefault();
      first.focus();
    }
  }

  return (
    <>
      <div className={styles.sheetScrim} onClick={onClose} aria-hidden="true" />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Filters"
        tabIndex={-1}
        ref={ref}
        className={styles.sheet}
        onKeyDown={onKeyDown}
      >
        <div className={styles.sheetHead}>
          <span className={styles.sheetTitle}>Filters</span>
          <button
            type="button"
            className={styles.sheetClose}
            aria-label="Close filters"
            onClick={onClose}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </div>
        {children}
      </div>
    </>
  );
}
