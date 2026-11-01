import { ChevronRight } from "lucide-react";
import { useId, useState, type ReactNode } from "react";
import styles from "./Panel.module.css";

interface PanelProps {
  title?: string;
  titleId?: string;
  /** Right of the title: a link or a figure. */
  aside?: ReactNode;
  /** Under the title, shown even when the panel is collapsed. */
  intro?: ReactNode;
  /** The title toggles the body open and closed. Needs a title. */
  collapsible?: boolean;
  /** With `collapsible`: whether the body starts open. */
  defaultOpen?: boolean;
  /** With `collapsible`: opens the body whenever this turns true (an error the reader must see). */
  forceOpen?: boolean;
  children: ReactNode;
}

export function Panel({
  title,
  titleId,
  aside,
  intro,
  collapsible = false,
  defaultOpen = true,
  forceOpen = false,
  children,
}: PanelProps) {
  const [open, setOpen] = useState(defaultOpen || forceOpen);
  // Opens when forceOpen turns true, adjusted during render rather than in an effect.
  const [wasForced, setWasForced] = useState(forceOpen);
  if (forceOpen !== wasForced) {
    setWasForced(forceOpen);
    if (forceOpen) setOpen(true);
  }
  const bodyId = useId();
  const foldable = collapsible && Boolean(title);
  const shown = !foldable || open;
  return (
    <section
      className={styles.panel}
      aria-labelledby={title ? titleId : undefined}
    >
      {title && (
        <div
          className={`${styles.head} ${foldable && !open && !intro ? styles.headClosed : ""}`}
        >
          {foldable ? (
            <h2 id={titleId} className={styles.title}>
              <button
                type="button"
                className={styles.toggle}
                aria-expanded={open}
                aria-controls={bodyId}
                onClick={() => setOpen(!open)}
              >
                <ChevronRight
                  size={14}
                  aria-hidden="true"
                  className={`${styles.chevron} ${open ? styles.chevronOpen : ""}`}
                />
                {title}
              </button>
            </h2>
          ) : (
            <h2 id={titleId} className={styles.title}>
              {title}
            </h2>
          )}
          {aside}
        </div>
      )}
      {intro && (
        <div
          className={`${styles.intro} ${foldable && !open ? styles.introClosed : ""}`}
        >
          {intro}
        </div>
      )}
      {foldable ? (
        <div id={bodyId} hidden={!shown}>
          {shown && children}
        </div>
      ) : (
        children
      )}
    </section>
  );
}
