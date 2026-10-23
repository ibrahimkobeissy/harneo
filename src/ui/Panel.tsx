import type { ReactNode } from "react";
import styles from "./Panel.module.css";

interface PanelProps {
  title?: string;
  titleId?: string;
  /** Right of the title: a link or a figure. */
  aside?: ReactNode;
  children: ReactNode;
}

export function Panel({ title, titleId, aside, children }: PanelProps) {
  return (
    <section
      className={styles.panel}
      aria-labelledby={title ? titleId : undefined}
    >
      {title && (
        <div className={styles.head}>
          <h2 id={titleId} className={styles.title}>
            {title}
          </h2>
          {aside}
        </div>
      )}
      {children}
    </section>
  );
}
