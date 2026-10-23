import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import styles from "./EmptyState.module.css";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  children?: ReactNode;
  /** A button under the text: the way out of the state. */
  action?: ReactNode;
}

export function EmptyState({
  icon: Icon,
  title,
  children,
  action,
}: EmptyStateProps) {
  return (
    <div className={styles.empty}>
      <span className={styles.ico}>
        <Icon size={20} aria-hidden="true" />
      </span>
      <h2 className={styles.title}>{title}</h2>
      {children && <p className={styles.text}>{children}</p>}
      {action}
    </div>
  );
}
