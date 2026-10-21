import type { ReactNode } from "react";
import styles from "./Screen.module.css";

// A page: one h1 and its content. The h1 labels the section landmark.
export function Screen({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  const id = `${title.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-title`;
  return (
    <section className={styles.page} aria-labelledby={id}>
      <header className={styles.head}>
        <h1 id={id} className={styles.title}>
          {title}
        </h1>
      </header>
      {children}
    </section>
  );
}
