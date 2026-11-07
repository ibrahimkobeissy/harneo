import { useEffect, useId, useRef, useState } from "react";
import { Button, Kbd } from "../../ui";
import styles from "./Harnmap.module.css";
import { SentLine } from "./SentLine";

/**
 * The Re-draw dialog: the description, what stays put, what is sent, Draw again. The current map
 * stays visible behind it, and while Claude works.
 */
export function RedrawDialog({
  initial,
  pinned,
  count,
  onDraw,
  onClose,
}: {
  initial: string;
  /** Names of the pinned artifacts. */
  pinned: string[];
  /** How many artifacts are sent. */
  count: number;
  onDraw: (description: string) => void;
  onClose: () => void;
}) {
  const [text, setText] = useState(initial);
  const area = useRef<HTMLTextAreaElement>(null);
  const titleId = useId();
  const fieldId = useId();

  useEffect(() => {
    area.current?.focus();
  }, []);

  return (
    <div
      className={styles.scrim}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div
        className={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <h2 id={titleId} className={styles.dialogTitle}>
          Re-draw the map
        </h2>
        <label className={styles.label} htmlFor={fieldId}>
          How do you see this factory?
        </label>
        <textarea
          id={fieldId}
          ref={area}
          className={styles.textarea}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) onDraw(text);
          }}
        />
        <div className={styles.pinned}>
          <b>Stays put: {pinned.length} pinned</b>
          {pinned.length > 0 && (
            <ul aria-label="Pinned artifacts">
              {pinned.map((name) => (
                <li key={name}>
                  <code>{name}</code>
                </li>
              ))}
            </ul>
          )}
        </div>
        <SentLine count={count} />
        <p className={styles.muted}>
          Your current map stays visible while Claude works, and a re-draw is
          added to Map history; nothing is overwritten.
        </p>
        <div className={styles.row}>
          <Button variant="primary" onClick={() => onDraw(text)}>
            Draw again <Kbd>Ctrl ↵</Kbd>
          </Button>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}
