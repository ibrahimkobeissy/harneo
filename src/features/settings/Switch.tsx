import styles from "./Settings.module.css";

/** A 34x20 toggle: a button with role switch, named by its `id`'s label. */
export function Switch({
  id,
  checked,
  disabled = false,
  onChange,
}: {
  id: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (next: boolean) => void;
}) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      className={styles.switch}
      onClick={() => onChange(!checked)}
    >
      <i />
    </button>
  );
}
