import type { ReactNode } from "react";
import styles from "./Badge.module.css";

export type BadgeTone = "neutral" | "ok" | "warn" | "bad" | "info";

export function Badge({
  tone = "neutral",
  children,
}: {
  tone?: BadgeTone;
  children: ReactNode;
}) {
  return (
    <span className={`${styles.badge} ${styles[tone]}`} data-tone={tone}>
      {children}
    </span>
  );
}
