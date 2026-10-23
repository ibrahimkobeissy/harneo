import { useEffect, useState } from "react";
import { settingsSet } from "../../lib/ipc";
import { readAutoCheck } from "../../lib/updates";
import { Panel } from "../../ui";
import styles from "./UpdatesPanel.module.css";

/** `updates.autoCheck`: whether the app looks at the library's sources when it starts. */
export function UpdatesPanel() {
  const [on, setOn] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    readAutoCheck().then(
      (value) => {
        if (current) setOn(value);
      },
      () => {
        if (current) setOn(true);
      },
    );
    return () => {
      current = false;
    };
  }, []);

  async function change(next: boolean) {
    const previous = on;
    setOn(next);
    setError(null);
    try {
      await settingsSet("updates.autoCheck", next);
    } catch (err) {
      setOn(previous);
      setError(String(err));
    }
  }

  return (
    <div className={styles.wrap}>
      <Panel title="Updates" titleId="updates-title">
        <label className={styles.row}>
          <input
            type="checkbox"
            checked={on ?? true}
            disabled={on === null}
            onChange={(e) => void change(e.target.checked)}
          />
          <span>Check for updates at start</span>
        </label>
        <p className={styles.note}>
          Once a day, in the background, Harneo looks at each source for
          artifacts that changed since you imported them. It only uses git;
          nothing is updated without you.
        </p>
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </Panel>
    </div>
  );
}
