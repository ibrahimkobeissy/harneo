import type { Status } from "./format";
import styles from "./Projects.module.css";

/** The project's state as a dot and a word; colour is never the only signal. */
export function StatusPill({ status }: { status: Status }) {
  // Slot for the "Drift · N files" pill (DR3): another `status.kind` and branch here.
  const tone = status.kind === "available" ? "blue" : "green";
  const text =
    status.kind === "available"
      ? `Profile v${status.version} available`
      : "In sync";
  return (
    <span className={styles.status} data-tone={tone}>
      <i className={styles.dot} aria-hidden="true" />
      {text}
    </span>
  );
}
