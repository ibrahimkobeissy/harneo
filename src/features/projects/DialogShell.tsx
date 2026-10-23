import { useEffect, type FormEvent, type ReactNode } from "react";
import { Modal } from "../../app/Modal";
import { Button, type ButtonVariant } from "../../ui";
import styles from "./Projects.module.css";

/**
 * A modal with a title, a scrolling body, Cancel and one action. Esc, the scrim and Cancel close
 * it, except while `busy`: a running apply cannot be walked away from.
 */
export function DialogShell({
  title,
  sub,
  submit,
  variant = "primary",
  busy,
  canSubmit = true,
  wide = false,
  onSubmit,
  onClose,
  children,
}: {
  title: string;
  sub?: string;
  submit: string;
  variant?: ButtonVariant;
  busy: boolean;
  canSubmit?: boolean;
  wide?: boolean;
  onSubmit: () => void;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    function onKeyDown(ev: KeyboardEvent) {
      if (ev.key === "Escape" && !ev.defaultPrevented && !busy) onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose, busy]);

  function submitForm(ev: FormEvent) {
    ev.preventDefault();
    onSubmit();
  }
  return (
    <Modal label={title} wide={wide} onClose={busy ? undefined : onClose}>
      <form className={styles.dlg} onSubmit={submitForm}>
        <div className={styles.dhead}>
          <h2>{title}</h2>
          {sub && <p>{sub}</p>}
        </div>
        <div className={styles.dbody}>{children}</div>
        <div className={styles.dfoot}>
          <Button disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button variant={variant} type="submit" disabled={busy || !canSubmit}>
            {submit}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
