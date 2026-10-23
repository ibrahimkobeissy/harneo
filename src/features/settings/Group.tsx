import type { ReactNode } from "react";
import styles from "./Settings.module.css";

/** One Settings panel: an icon and title in the header, rows below. */
export function Group({
  id,
  title,
  icon,
  children,
}: {
  id: string;
  title: string;
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={styles.group} aria-labelledby={id}>
      <header className={styles.ghead}>
        {icon}
        <h2 id={id}>{title}</h2>
      </header>
      {children}
    </section>
  );
}
