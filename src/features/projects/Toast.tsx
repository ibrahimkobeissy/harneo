import { useEffect, useState } from "react";
import styles from "./Projects.module.css";

export interface Notice {
  text: string;
  tone: "ok" | "bad";
  /** A folder the user may need later: the toast stays and offers copy and open. */
  folder?: string;
}

const NOTICE_MS = 10000;

/** A floating message. A success leaves by itself, unless it names a folder; an error stays. */
export function Toast({
  notice,
  onDismiss,
  onOpenFolder,
}: {
  notice: Notice;
  onDismiss: () => void;
  onOpenFolder?: (folder: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const folder = notice.folder;
  const sticky = notice.tone === "bad" || folder !== undefined;
  useEffect(() => {
    if (sticky) return;
    const timer = setTimeout(onDismiss, NOTICE_MS);
    return () => clearTimeout(timer);
  }, [sticky, notice, onDismiss]);

  async function copy(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <div
      className={styles.toast}
      data-tone={notice.tone}
      role={notice.tone === "bad" ? "alert" : "status"}
    >
      <span className={styles.toastText}>
        {notice.text}
        {folder && (
          <span className={`${styles.mono} ${styles.toastPath}`}>{folder}</span>
        )}
      </span>
      {folder && (
        <>
          <button
            type="button"
            className={styles.toastClose}
            onClick={() => void copy(folder)}
          >
            {copied ? "Copied" : "Copy path"}
          </button>
          {onOpenFolder && (
            <button
              type="button"
              className={styles.toastClose}
              onClick={() => onOpenFolder(folder)}
            >
              Open backup folder
            </button>
          )}
        </>
      )}
      <button type="button" className={styles.toastClose} onClick={onDismiss}>
        Dismiss
      </button>
    </div>
  );
}
