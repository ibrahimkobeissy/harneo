import { useEffect, useId, useState } from "react";
import type { WatchStatus } from "../../bindings/WatchStatus";
import { watchPause, watchStatus } from "../../lib/ipc";
import styles from "./Settings.module.css";
import { Switch } from "./Switch";

/** Live rescan: how many projects are watched, and a switch to pause it (`watch.paused`); a row of Background. */
export function LiveRescan() {
  const [status, setStatus] = useState<WatchStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const toggleId = useId();
  const helpId = useId();
  useEffect(() => {
    let current = true;
    watchStatus().then(
      (value) => {
        if (current) setStatus(value);
      },
      (err: unknown) => {
        if (current) setError(String(err));
      },
    );
    return () => {
      current = false;
    };
  }, []);

  async function change(paused: boolean) {
    if (status === null) return;
    const previous = status;
    setStatus({ ...status, paused });
    setError(null);
    try {
      await watchPause(paused);
    } catch (err) {
      setStatus(previous);
      setError(String(err));
    }
  }

  const count = status?.watching ?? 0;
  return (
    <>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={toggleId}>
            <b>Pause live rescan</b>
          </label>
          <div id={helpId} className={styles.sec}>
            {status === null
              ? "Looking…"
              : `Watching ${count} ${count === 1 ? "project" : "projects"}`}
            {status?.paused && " (paused)"}. Harneo notices changes in your
            library and in each project&apos;s harness files; it only reads.
          </div>
        </div>
        <Switch
          id={toggleId}
          checked={status?.paused ?? false}
          disabled={status === null}
          describedBy={helpId}
          onChange={(next) => void change(next)}
        />
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </>
  );
}
