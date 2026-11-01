import { CircleCheck, FolderGit2, RotateCcw } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router";
import { Button, PluginsInstall } from "../../ui";
import styles from "./Refit.module.css";

/** The summary after the refit, with a way back to the originals. */
export function DoneStep({
  folder,
  profile,
  backedUp,
  backupFolder,
  projectId,
  restoredCount,
  addedCount,
  plugins,
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
  backupFolder: string | null;
  projectId: string;
  /** Files the restore put back, once it ran. */
  restoredCount: number;
  /** Files the refit added; a restore leaves them in place. */
  addedCount: number;
  /** The profile holds plugins, which are enabled in settings but never copied. */
  plugins: boolean;
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
  // The actions wait while the plugins install.
  const [installing, setInstalling] = useState(plugins);
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
      {plugins && (
        <PluginsInstall
          projectId={BigInt(projectId)}
          onRunningChange={setInstalling}
        />
      )}
      {backupFolder && (
        <p>
          Backup saved in <span className={styles.mono}>{backupFolder}</span>.
        </p>
      )}
      {restored && (
        <p role="status" className={styles.okNote}>
          Originals restored: {restoredCount}{" "}
          {restoredCount === 1 ? "file" : "files"} put back, {addedCount} added{" "}
          {addedCount === 1 ? "file remains" : "files remain"}. The project
          stays on {profile} and may now show drift.
        </p>
      )}
      <p>
        {installing ? (
          <span className={styles.muted}>
            Open the project to see its Backups tab
          </span>
        ) : (
          <Link to={`/projects/${projectId}`}>
            Open the project to see its Backups tab
          </Link>
        )}
      </p>
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
          <Button onClick={onAskRestore} disabled={installing}>
            <RotateCcw size={14} aria-hidden="true" />
            Restore originals
          </Button>
        )}
        <Button variant="primary" onClick={onProjects} disabled={installing}>
          <FolderGit2 size={14} aria-hidden="true" />
          Go to projects
        </Button>
      </div>
    </div>
  );
}
