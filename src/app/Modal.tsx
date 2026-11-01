import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import styles from "./Modal.module.css";

interface ModalProps {
  label: string;
  variant?: "dialog" | "palette";
  /** Room for side-by-side content (diffs). */
  wide?: boolean;
  onClose?: () => void;
  children: ReactNode;
}

const focusable = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

// Accessible modal: labelled, focus moves in on open, is trapped on Tab and
// returns on close. Esc is handled by the Layout key handler; a click on the
// scrim calls onClose. It renders into document.body, so no ancestor with a mask, transform or
// overflow can clip or reposition it.
export function Modal({
  label,
  variant = "dialog",
  wide = false,
  onClose,
  children,
}: ModalProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement;
    const node = ref.current;
    if (node && !node.contains(document.activeElement)) {
      (node.querySelector<HTMLElement>(focusable) ?? node).focus();
    }
    return () => {
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);

  function trapTab(ev: KeyboardEvent<HTMLDivElement>) {
    if (ev.key !== "Tab") return;
    const node = ref.current;
    if (!node) return;
    const items = Array.from(node.querySelectorAll<HTMLElement>(focusable));
    if (items.length === 0) {
      ev.preventDefault();
      node.focus();
      return;
    }
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

  return createPortal(
    <>
      <div className={styles.scrim} onClick={onClose} aria-hidden="true" />
      <div
        role="dialog"
        aria-modal="true"
        aria-label={label}
        tabIndex={-1}
        ref={ref}
        className={`${styles.box} ${variant === "palette" ? styles.palette : styles.dialog} ${wide ? styles.wide : ""}`}
        onKeyDown={trapTab}
      >
        {children}
      </div>
    </>,
    document.body,
  );
}
