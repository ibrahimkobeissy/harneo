import type { ReactNode } from "react";
import styles from "./Screen.module.css";

/**
 * A page: a fixed header (crumb, the h1 row, the screen's toolbar) above the one scrolling body.
 * The h1 labels the section landmark.
 */
export function Screen({
  title,
  narrow = false,
  crumb,
  icon,
  badge,
  actions,
  toolbar,
  foot,
  children,
}: {
  title: string;
  /** Narrow flows (settings, forms) cap at 1040px, per DESIGN.md. */
  narrow?: boolean;
  /** A back link above the title. */
  crumb?: ReactNode;
  /** Decoration before the title text (hidden from assistive tech by the caller). */
  icon?: ReactNode;
  /** Shown right after the title. */
  badge?: ReactNode;
  /** The title row's controls (search, view toggle, primary action). */
  actions?: ReactNode;
  /** Pinned under the title row: bars, chips, steppers. */
  toolbar?: ReactNode;
  /** Pinned under the scrolling body: a wizard's action bar. */
  foot?: ReactNode;
  children: ReactNode;
}) {
  const id = `${title.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-title`;
  const cap = narrow ? `${styles.in} ${styles.narrow}` : styles.in;
  return (
    <section className={styles.page} aria-labelledby={id}>
      <div className={styles.head} data-screen-head="">
        <div className={cap}>
          {crumb}
          <header className={styles.row}>
            <h1 id={id} className={styles.title}>
              {icon}
              {title}
            </h1>
            {badge}
            {actions}
          </header>
          {toolbar}
        </div>
      </div>
      <div className={styles.body} data-screen-body="">
        <div className={cap}>{children}</div>
      </div>
      {foot && (
        <div className={styles.foot} data-screen-foot="">
          <div className={cap}>{foot}</div>
        </div>
      )}
    </section>
  );
}
