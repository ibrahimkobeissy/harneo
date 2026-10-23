import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import styles from "./Callout.module.css";

/** A tinted note with an icon. A `bad` one is announced as an alert. */
export function Callout({
  tone = "info",
  icon: Icon,
  children,
}: {
  tone?: "info" | "warn" | "bad";
  icon: LucideIcon;
  children: ReactNode;
}) {
  return (
    <div
      className={styles.callout}
      data-tone={tone}
      role={tone === "bad" ? "alert" : undefined}
    >
      <Icon size={16} aria-hidden="true" />
      <div className={styles.body}>{children}</div>
    </div>
  );
}
