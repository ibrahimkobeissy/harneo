import { CircleCheck, FolderGit2, RotateCcw } from "lucide-react";
import { Button } from "../../ui";
import styles from "./Refit.module.css";

/** The summary after the refit, with a way back to the originals. */
export function DoneStep({
  folder,
  profile,
  backedUp,
  restoring,
  restored,
  confirming,
  error,
  canRestore,
  onAskRestore,
  onCancel,
  onRestore,
  onProjects,
}: {
  folder: string;
  profile: string;
  backedUp: number;
  restoring: boolean;
  restored: boolean;
  confirming: boolean;
  error: string | null;
  canRestore: boolean;
  onAskRestore: () => void;
  onCancel: () => void;
  onRestore: () => void;
  onProjects: () => void;
}) {
  const originals = backedUp === 1 ? "original" : "originals";
  return (
    <div className={styles.result}>
      <div className={styles.okIcon}>
        <CircleCheck size={22} aria-hidden="true" />
      </div>
      <h2>
        Refitted {folder} with {profile}
      </h2>
      <p>
        {backedUp} {originals} backed up. The project is in your registry with a
        fresh <span className={styles.mono}>HARNESS.md</span>.
      </p>
      {restored && (
        <p role="status" className={styles.okNote}>
          Originals restored. The files you replaced are back.
        </p>
      )}
      {error && (
        <p role="alert" className={styles.errNote}>
          {error}
        </p>
      )}
      {confirming && (
        <div
          className={styles.confirm}
          role="group"
          aria-label="Confirm restore"
        >
          <span>
            Put the {backedUp} {originals} back? The refitted versions are
            backed up first.
          </span>
          <Button small onClick={onCancel} disabled={restoring}>
            Cancel
          </Button>
          <Button
            small
            variant="primary"
            onClick={onRestore}
            disabled={restoring}
          >
            {restoring ? "Restoring…" : "Restore"}
          </Button>
        </div>
      )}
      <div className={styles.actions}>
        {canRestore && !restored && !confirming && (
          <Button onClick={onAskRestore}>
            <RotateCcw size={14} aria-hidden="true" />
            Restore originals
          </Button>
        )}
        <Button variant="primary" onClick={onProjects}>
          <FolderGit2 size={14} aria-hidden="true" />
          Go to projects
        </Button>
      </div>
    </div>
  );
}
