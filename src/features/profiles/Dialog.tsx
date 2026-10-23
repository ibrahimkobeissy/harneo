import { useEffect, useId, type FormEvent, type ReactNode } from "react";
import { Modal } from "../../app/Modal";
import { Button } from "../../ui";
import styles from "./Profiles.module.css";

/** A modal form: title, scrolling body, Cancel and a submit button. Esc and the scrim close it. */
export function Dialog({
  title,
  sub,
  submit,
  busy,
  canSubmit = true,
  danger = false,
  cancel = "Cancel",
  onSubmit,
  onClose,
  children,
}: {
  title: string;
  sub?: string;
  submit: string;
  busy: boolean;
  canSubmit?: boolean;
  /** A destructive confirm: the submit button is red. */
  danger?: boolean;
  /** The dismiss button's label ("Stay" for the leave guard). */
  cancel?: string;
  onSubmit: () => void;
  onClose: () => void;
  children?: ReactNode;
}) {
  // The Modal leaves Esc to its owner.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !event.defaultPrevented) onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  function submitForm(event: FormEvent) {
    event.preventDefault();
    onSubmit();
  }
  return (
    <Modal label={title} onClose={onClose}>
      <form className={styles.dlg} onSubmit={submitForm}>
        <div className={styles.dhead}>
          <h2>{title}</h2>
          {sub && <p>{sub}</p>}
        </div>
        {children && <div className={styles.dbody}>{children}</div>}
        <div className={styles.dfoot}>
          <Button onClick={onClose}>{cancel}</Button>
          <Button
            variant={danger ? "danger-solid" : "primary"}
            type="submit"
            disabled={busy || !canSubmit}
          >
            {submit}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

export function NameField({
  value,
  onChange,
}: {
  value: string;
  onChange: (name: string) => void;
}) {
  const id = useId();
  return (
    <div className={styles.field}>
      <label htmlFor={id}>Name</label>
      <input
        id={id}
        className={styles.input}
        value={value}
        autoComplete="off"
        maxLength={60}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
