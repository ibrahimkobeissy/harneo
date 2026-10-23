import { CircleAlert, FolderSearch, RefreshCw } from "lucide-react";
import { Button } from "../../ui";
import styles from "./Scaffold.module.css";

/** Step 1: where the project goes. A refused folder comes back as `error`. */
export function FolderStep({
  path,
  error,
  onChange,
  onBrowse,
  onRefit,
}: {
  path: string;
  error: { message: string; hasHarness: boolean } | null;
  onChange: (path: string) => void;
  onBrowse: () => void;
  onRefit: () => void;
}) {
  return (
    <div className={styles.field}>
      <label htmlFor="scaffold-path">New project folder</label>
      <div className={styles.inputRow}>
        <input
          id="scaffold-path"
          className={styles.input}
          value={path}
          placeholder="~/work/new-app"
          aria-invalid={error ? true : undefined}
          aria-describedby="scaffold-path-note"
          onChange={(e) => onChange(e.target.value)}
        />
        <Button onClick={onBrowse}>
          <FolderSearch size={14} aria-hidden="true" />
          Browse…
        </Button>
      </div>
      {error ? (
        <>
          <span className={styles.err} id="scaffold-path-note" role="alert">
            <CircleAlert size={13} aria-hidden="true" />
            {error.message}
          </span>
          {error.hasHarness && (
            <Button small className={styles.refit} onClick={onRefit}>
              <RefreshCw size={13} aria-hidden="true" />
              Refit it instead
            </Button>
          )}
        </>
      ) : (
        <span className={styles.hint} id="scaffold-path-note">
          An empty folder, or a project with no Claude harness yet. Existing
          harnesses are refitted, not scaffolded.
        </span>
      )}
    </div>
  );
}
