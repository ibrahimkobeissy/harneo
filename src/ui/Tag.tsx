import type { ReactNode } from "react";
import styles from "./Tag.module.css";

// Tags never use green, amber, red or the accent blue: those mean state or selection.
export type TagTint =
  | "cyan"
  | "sky"
  | "indigo"
  | "violet"
  | "fuchsia"
  | "stone"
  | "slate"
  | "plain"
  | "ink";

export function Tag({
  tint = "slate",
  dot = false,
  children,
}: {
  tint?: TagTint;
  dot?: boolean;
  children: ReactNode;
}) {
  return (
    <span className={styles.tag} data-tint={tint}>
      {dot && <i className={styles.dot} aria-hidden="true" />}
      {children}
    </span>
  );
}
