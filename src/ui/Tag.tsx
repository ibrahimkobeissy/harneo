import type { ReactNode } from "react";
import styles from "./Tag.module.css";

// Tags never use green or the accent blue. Red, amber and emerald are soft washes, distinct from the state colours (see DESIGN.md).
export type TagTint =
  | "cyan"
  | "sky"
  | "indigo"
  | "violet"
  | "fuchsia"
  | "stone"
  | "slate"
  | "red"
  | "orange"
  | "amber"
  | "lime"
  | "emerald"
  | "teal"
  | "pink"
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
