import type { ReactNode } from "react";
import styles from "./Panel.module.css";

interface PanelProps {
  title?: string;
  titleId?: string;
  children: ReactNode;
}

export function Panel({ title, titleId, children }: PanelProps) {
  return (
    <section
      className={styles.panel}
      aria-labelledby={title ? titleId : undefined}
    >
      {title && (
        <h2 id={titleId} className={styles.title}>
          {title}
        </h2>
      )}
      {children}
    </section>
  );
}
