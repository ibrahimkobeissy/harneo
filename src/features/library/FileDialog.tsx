import { ExternalLink, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { FileView } from "../../bindings/FileView";
import { Modal } from "../../app/Modal";
import { libraryOpenFile, libraryReadFile } from "../../lib/ipc";
import { Button, FileViewer } from "../../ui";
import styles from "./Detail.module.css";
import { useEscape } from "./useEscape";

/** A popup with one file of an artifact's library copy, read-only. */
export function FileDialog({
  id,
  path,
  onClose,
}: {
  id: string;
  path: string;
  onClose: () => void;
}) {
  const [view, setView] = useState<FileView | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEscape(onClose);

  useEffect(() => {
    let live = true;
    libraryReadFile(id, path).then(
      (v) => live && setView(v),
      (e: unknown) => live && setError(String(e)),
    );
    return () => {
      live = false;
    };
  }, [id, path]);

  async function openInEditor() {
    try {
      await libraryOpenFile(id, path);
    } catch (e) {
      setError(String(e));
    }
  }

  const reason =
    view?.kind === "binary"
      ? "This file is not text, so there is no preview."
      : view?.kind === "tooLarge"
        ? "This file is over 1 MB, so there is no preview."
        : null;

  return (
    <Modal label={path} wide onClose={onClose}>
      <div className={styles.fileHead}>
        <h2 className={styles.fileTitle}>{path}</h2>
        <Button
          title="Scripts and programs are shown in the file manager instead, so they never run"
          onClick={() => void openInEditor()}
        >
          <ExternalLink size={13} aria-hidden="true" /> Open in editor
        </Button>
        <button
          type="button"
          className={styles.fileClose}
          aria-label="Close"
          onClick={onClose}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
      {error !== null && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      {view === null ? (
        error === null && <p className={styles.muted}>Loading…</p>
      ) : (
        <FileViewer
          path={path}
          text={view.kind === "text" ? view.text : null}
          empty={<p>{reason}</p>}
        />
      )}
    </Modal>
  );
}
