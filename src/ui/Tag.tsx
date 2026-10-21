import type { ReactNode } from "react";
import styles from "./Tag.module.css";

export type TagTint =
  | "slate"
  | "blue"
  | "violet"
  | "cyan"
  | "orange"
  | "teal"
  | "pink"
  | "indigo"
  | "amber"
  | "green"
  | "red"
  | "sky"
  | "fuchsia"
  | "stone";

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
