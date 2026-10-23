import { RefreshCw } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { settingsSet } from "../../lib/ipc";
import { readAutoCheck } from "../../lib/updates";
import { Group } from "./Group";
import styles from "./Settings.module.css";
import { Switch } from "./Switch";

/** `updates.autoCheck`: whether the app looks at the library's sources when it starts. */
export function UpdatesPanel() {
  const [on, setOn] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const toggleId = useId();
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
    <Group
      id="updates-title"
      title="Updates"
      icon={<RefreshCw size={15} aria-hidden="true" />}
    >
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={toggleId}>
            <b>Check for updates at start</b>
          </label>
          <div className={styles.sec}>
            Once a day, in the background, Harneo looks at each source for
            artifacts that changed since you imported them. It only uses git;
            nothing is updated without you.
          </div>
        </div>
        <Switch
          id={toggleId}
          checked={on ?? true}
          disabled={on === null}
          onChange={(next) => void change(next)}
        />
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Group>
  );
}
