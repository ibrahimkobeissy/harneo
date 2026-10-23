import { useEffect } from "react";
import styles from "./Projects.module.css";

export interface Notice {
  text: string;
  tone: "ok" | "bad";
}

const NOTICE_MS = 6000;

/** A floating message. A success leaves by itself; an error stays until dismissed. */
export function Toast({
  notice,
  onDismiss,
}: {
  notice: Notice;
  onDismiss: () => void;
}) {
  useEffect(() => {
    if (notice.tone === "bad") return;
    const timer = setTimeout(onDismiss, NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice, onDismiss]);
  return (
    <div
      className={styles.toast}
      data-tone={notice.tone}
      role={notice.tone === "bad" ? "alert" : "status"}
    >
      <span className={styles.toastText}>{notice.text}</span>
      <button type="button" className={styles.toastClose} onClick={onDismiss}>
        Dismiss
      </button>
    </div>
  );
}
