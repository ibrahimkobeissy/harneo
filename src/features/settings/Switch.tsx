import styles from "./Settings.module.css";

/** A 34x20 toggle: a button with role switch, named by its `id`'s label, with On or Off beside it. */
export function Switch({
  id,
  checked,
  disabled = false,
  describedBy,
  onChange,
}: {
  id: string;
  checked: boolean;
  disabled?: boolean;
  /** The id of the row's description, read with the switch. */
  describedBy?: string;
  onChange: (next: boolean) => void;
}) {
  return (
    <span className={styles.switchWrap}>
      <span className={styles.switchText} aria-hidden="true">
        {checked ? "On" : "Off"}
      </span>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-describedby={describedBy}
        disabled={disabled}
        className={styles.switch}
        onClick={() => onChange(!checked)}
      >
        <i />
      </button>
    </span>
  );
}
