import { plural, type Status } from "./format";
import styles from "./Projects.module.css";

function look(status: Status): { tone: string; text: string } {
  switch (status.kind) {
    case "drift":
      return { tone: "amber", text: `Drift · ${plural(status.files, "file")}` };
    case "available":
      return { tone: "blue", text: `Profile v${status.version} available` };
    case "library":
      return { tone: "blue", text: "Library has newer copies" };
    case "missing":
      return { tone: "red", text: "Missing" };
    case "sync":
      return { tone: "green", text: "In sync" };
  }
}

/** The project's state as a dot and a word; colour is never the only signal. */
export function StatusPill({ status }: { status: Status }) {
  const { tone, text } = look(status);
  return (
    <span className={styles.status} data-tone={tone}>
      <i className={styles.dot} aria-hidden="true" />
      {text}
    </span>
  );
}
