import type { ReactNode } from "react";
import styles from "./WizardActions.module.css";

/**
 * The wizards' action bar, passed to Screen's `foot`: Back (or a skip) on the left, the primary
 * action on the right. DOM order is the keyboard order: back, then primary.
 */
export function WizardActions({
  back,
  note,
  children,
}: {
  back?: ReactNode;
  /** Quiet text before the primary action; it truncates before the buttons shrink. */
  note?: ReactNode;
  /** The primary action (and anything beside it). */
  children?: ReactNode;
}) {
  return (
    <div className={styles.bar}>
      {back}
      <span className={styles.spacer} />
      {note && <span className={styles.note}>{note}</span>}
      {children}
    </div>
  );
}
