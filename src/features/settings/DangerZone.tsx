import { TriangleAlert } from "lucide-react";
import { useId, useState, type FormEvent } from "react";
import { Modal } from "../../app/Modal";
import { resetHarneo } from "../../lib/ipc";
import { Button } from "../../ui";
import { Group } from "./Group";
import styles from "./Settings.module.css";

/** What happens once the data is gone: back to the empty Library, with every screen reloaded. */
export function reloadToLibrary() {
  window.location.hash = "#/library";
  window.location.reload();
}

/** The word to type before the reset button works. */
const CONFIRM_WORD = "reset";

/** Danger zone: reset all of Harneo's data. Shown last in Settings, in red. */
export function DangerZone({
  onDone = reloadToLibrary,
}: {
  onDone?: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Group
        id="danger"
        title="Danger zone"
        icon={<TriangleAlert size={15} aria-hidden="true" />}
        className={styles.danger}
      >
        <div className={styles.row}>
          <div className={styles.grow}>
            <b>Reset Harneo</b>
            <div className={styles.sec}>
              Deletes everything Harneo keeps and starts again with an empty
              library.
            </div>
          </div>
          <Button variant="danger" onClick={() => setOpen(true)}>
            Reset Harneo…
          </Button>
        </div>
      </Group>
      {open && <ResetDialog onClose={() => setOpen(false)} onDone={onDone} />}
    </>
  );
}

function ResetDialog({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: () => void;
}) {
  const [typed, setTyped] = useState("");
  const [backups, setBackups] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputId = useId();
  const ready = typed.trim() === CONFIRM_WORD;

  async function run(event: FormEvent) {
    event.preventDefault();
    if (!ready || busy) return;
    setBusy(true);
    setError(null);
    try {
      await resetHarneo(backups);
      onDone();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }

  return (
    <Modal label="Reset Harneo" onClose={busy ? undefined : onClose}>
      <form
        className={styles.dlg}
        onSubmit={(e) => void run(e)}
        aria-busy={busy}
      >
        <div className={styles.dhead}>
          <h2>Reset Harneo</h2>
        </div>
        <div className={styles.dbody}>
          <p>This permanently deletes:</p>
          <ul>
            <li>the library: artifacts, tags and collections</li>
            <li>all profiles</li>
            <li>the project registry and drift data</li>
            <li>the usage index</li>
            <li>Harneo&apos;s settings</li>
          </ul>
          <p>
            Your project files and <code>~/.claude</code> are never touched.
          </p>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={backups}
              disabled={busy}
              onChange={(e) => setBackups(e.target.checked)}
            />
            Also delete backups
          </label>
          {backups ? (
            <p className={styles.dwarn} role="note">
              Backups hold the originals of files Harneo replaced in your
              projects. Without them those originals cannot be restored.
            </p>
          ) : (
            <p>
              Backups are kept by default: they restore your projects&apos;
              original files.
            </p>
          )}
          <label htmlFor={inputId}>
            Type <b>{CONFIRM_WORD}</b> to confirm
          </label>
          <input
            id={inputId}
            value={typed}
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => setTyped(e.target.value)}
          />
          {error && (
            <p role="alert" className={styles.error}>
              {error}
            </p>
          )}
        </div>
        <div className={styles.dfoot}>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="danger-solid"
            type="submit"
            disabled={busy || !ready}
          >
            {busy ? "Resetting…" : "Reset Harneo"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
