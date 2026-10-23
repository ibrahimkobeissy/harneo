import { HardDrive } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useUsageIndex } from "../../app/usageIndex";
import { useAppInfo } from "../../app/useAppInfo";
import { settingsGet, settingsSet } from "../../lib/ipc";
import { Button, announce } from "../../ui";
import { CopyPath } from "./CopyPath";
import { Group } from "./Group";
import styles from "./Settings.module.css";

const WEEKS_KEY = "usage.unusedWeeks";
const LAST_KEY = "usage.lastIndexedAt";
const DEFAULT_WEEKS = 8;
const LOGS_PATH = "~/.claude/projects/";

function validWeeks(text: string): number | null {
  const n = Number(text);
  return Number.isInteger(n) && n >= 1 && n <= 52 ? n : null;
}

function when(iso: string | null): string {
  if (!iso) return "never";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "never" : date.toLocaleString();
}

/** Settings → Data: the data folder, the session logs Harneo reads, and the unused window. */
export function UsageSettings() {
  const app = useAppInfo();
  const { progress, runs, error: indexError, refresh } = useUsageIndex();
  const [last, setLast] = useState<string | null>(null);
  const [weeks, setWeeks] = useState(String(DEFAULT_WEEKS));
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const storedWeeks = useRef<number | null>(null);
  const weeksId = useId();
  const weeksHelpId = useId();

  useEffect(() => {
    let live = true;
    settingsGet(WEEKS_KEY).then(
      (v) => {
        if (live && typeof v === "number") {
          storedWeeks.current = v;
          setWeeks(String(v));
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  // `runs` changes when an index run completes, which rewrites the stored time.
  useEffect(() => {
    let live = true;
    settingsGet(LAST_KEY).then(
      (v) => {
        if (live) setLast(typeof v === "string" ? v : null);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [runs]);

  /** Saves the window once per commit (blur or Enter), and only when it changed. */
  async function commitWeeks() {
    const n = validWeeks(weeks);
    if (n === null || n === storedWeeks.current) return;
    setSaveError(null);
    try {
      await settingsSet(WEEKS_KEY, n);
      storedWeeks.current = n;
      setSaved(true);
      announce("Setting saved");
    } catch (err) {
      setSaveError(String(err));
    }
  }

  const invalid = validWeeks(weeks) === null;
  const running = progress !== null;
  const dataDir = app.status === "ready" ? app.info.dataDir : null;
  return (
    <Group
      id="usage-data"
      title="Data"
      icon={<HardDrive size={15} aria-hidden="true" />}
    >
      <div className={styles.row}>
        <div className={styles.grow}>
          <b>Data folder</b>
          <div className={styles.sec}>
            Library, profiles, project registry and backups.
          </div>
        </div>
        <span className={styles.mono}>{dataDir ?? "…"}</span>
        {dataDir && (
          <CopyPath path={dataDir} label="Copy the data folder path" />
        )}
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <b>Session logs (read only)</b>
          <div className={styles.sec}>
            Used for usage counts; never written.
          </div>
          <div className={styles.sec}>
            Last read: <span data-testid="last-indexed">{when(last)}</span>
            {running && (
              <>
                {" · "}
                <span role="status">
                  Reading logs
                  {progress.filesTotal > 0
                    ? `: ${progress.filesDone} of ${progress.filesTotal} files`
                    : "…"}
                </span>
              </>
            )}
          </div>
          <div className={styles.actions}>
            <Button small disabled={running} onClick={refresh}>
              {running ? "Indexing…" : "Re-index"}
            </Button>
          </div>
          {indexError && (
            <div className={styles.error} role="alert">
              Could not read the logs. {indexError}
            </div>
          )}
        </div>
        <span className={styles.mono}>{LOGS_PATH}</span>
        <CopyPath path={LOGS_PATH} label="Copy the session logs path" />
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={weeksId}>
            <b>Unused after N weeks</b>
          </label>
          <div id={weeksHelpId} className={styles.sec}>
            An artifact with no use for this long shows as unused (1 to 52).
            Saved when you leave the field or press Enter.
          </div>
          {invalid && (
            <div className={styles.error} role="alert">
              Enter a whole number from 1 to 52.
            </div>
          )}
        </div>
        <input
          id={weeksId}
          type="number"
          min={1}
          max={52}
          step={1}
          className={`${styles.select} ${styles.number}`}
          value={weeks}
          aria-invalid={invalid}
          aria-describedby={weeksHelpId}
          onChange={(event) => {
            setWeeks(event.target.value);
            setSaved(false);
          }}
          onBlur={() => void commitWeeks()}
          onKeyDown={(event) => {
            if (event.key === "Enter") void commitWeeks();
          }}
        />
        <span className={styles.savedNote}>{saved ? "Saved" : ""}</span>
      </div>
      {saveError && (
        <p role="alert" className={styles.error}>
          {saveError}
        </p>
      )}
    </Group>
  );
}
