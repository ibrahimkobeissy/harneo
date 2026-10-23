import { Archive, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import type { Backup } from "../../bindings/Backup";
import { backupDelete, backupRestore } from "../../lib/ipc";
import { Button } from "../../ui";
import { DialogShell } from "./DialogShell";
import { dateTimeOf, messageOf, plural } from "./format";
import styles from "./Projects.module.css";

type Ask = { kind: "restore" | "delete"; backup: Backup };

/** The backup sets of a project: restore one (current files are backed up first) or delete it. */
export function BackupsTab({
  backups,
  dataDir,
  onChanged,
  onNotice,
}: {
  backups: Backup[];
  dataDir: string | null;
  /** Reload after a restore or a delete. */
  onChanged: () => void;
  onNotice: (text: string, tone: "ok" | "bad") => void;
}) {
  const [ask, setAsk] = useState<Ask | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const first = backups[0]?.folder;
  const where = first?.includes("/")
    ? first.slice(0, first.lastIndexOf("/"))
    : "backups";

  async function confirm() {
    if (!ask) return;
    setBusy(true);
    setError(null);
    try {
      if (ask.kind === "restore") {
        const report = await backupRestore(ask.backup.id);
        onNotice(
          `Restored ${plural(report.written, "file")}. The current versions were backed up first.`,
          "ok",
        );
      } else {
        await backupDelete(ask.backup.id);
        onNotice("Deleted the backup.", "ok");
      }
      setAsk(null);
      onChanged();
    } catch (err) {
      setError(messageOf(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {backups.length === 0 ? (
        <p className={styles.muted}>No backups yet.</p>
      ) : (
        <ul className={styles.group} aria-label="Backups">
          {backups.map((b) => (
            <li key={String(b.id)} className={styles.row} title={b.folder}>
              <Archive
                size={16}
                aria-hidden="true"
                className={styles.rowIcon}
              />
              <div className={styles.grow}>
                <b>{b.reason}</b>
                <div className={styles.sec}>
                  {dateTimeOf(b.createdAt)} · {plural(b.fileCount, "file")}{" "}
                  saved
                </div>
              </div>
              <Button
                small
                onClick={() => setAsk({ kind: "restore", backup: b })}
              >
                <RotateCcw size={14} aria-hidden="true" />
                Restore
              </Button>
              <Button
                small
                variant="danger"
                aria-label={`Delete backup ${b.reason}`}
                onClick={() => setAsk({ kind: "delete", backup: b })}
              >
                <Trash2 size={14} aria-hidden="true" />
                Delete
              </Button>
            </li>
          ))}
        </ul>
      )}
      <p className={styles.footnote}>
        Backups live in{" "}
        <span className={styles.mono}>
          {dataDir ? `${dataDir}/${where}/` : `${where}/`}
        </span>
        . Harneo backs up every file before replacing it.
      </p>
      {ask && (
        <DialogShell
          title={
            ask.kind === "restore"
              ? "Restore this backup?"
              : "Delete this backup?"
          }
          submit={ask.kind === "restore" ? "Restore" : "Delete backup"}
          variant={ask.kind === "restore" ? "primary" : "danger-solid"}
          busy={busy}
          onSubmit={() => void confirm()}
          onClose={() => {
            setAsk(null);
            setError(null);
          }}
        >
          <p className={styles.text}>
            {ask.kind === "restore"
              ? `Puts back the ${plural(ask.backup.fileCount, "file")} saved by "${ask.backup.reason}". The files as they are now are backed up first.`
              : `Removes the ${plural(ask.backup.fileCount, "saved file")} of "${ask.backup.reason}" from disk. This cannot be undone.`}
          </p>
          {error && (
            <p className={styles.err} role="alert">
              {error}
            </p>
          )}
        </DialogShell>
      )}
    </>
  );
}
