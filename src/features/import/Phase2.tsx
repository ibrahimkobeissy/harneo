import { ChevronDown } from "lucide-react";
import { useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import { FindingTitle } from "../../ui";
import styles from "./Import.module.css";

/** "Confirming N of M with Claude…" while the background run is going. */
export function ConfirmingNote({
  progress,
}: {
  progress: ConfirmProgress | null;
}) {
  if (progress === null) return null;
  return (
    <p className={`${styles.muted} ${styles.counter}`} role="status">
      {`Confirming ${Math.min(progress.done + 1, progress.total)} of ${progress.total} with Claude…`}
    </p>
  );
}

/** "Claude: not a threat (N)": the findings Claude dismissed, with its reasons, on demand. */
export function DismissedNote({ candidate }: { candidate: Candidate }) {
  const [open, setOpen] = useState(false);
  const list = candidate.audit.dismissed;
  if (list.length === 0) return null;
  return (
    <span className={styles.block}>
      <button
        type="button"
        className={styles.linkButton}
        aria-expanded={open}
        aria-controls={`dismissed-${candidate.id}`}
        onClick={() => setOpen(!open)}
      >
        {`Claude: not a threat (${list.length})`}
        <ChevronDown size={12} aria-hidden="true" data-open={open} />
      </button>
      {open && (
        <ul className={styles.diags} id={`dismissed-${candidate.id}`}>
          {list.map((d, i) => (
            <li
              key={`${d.finding.rule}:${d.finding.file}:${d.finding.line ?? 0}:${i}`}
              className={styles.diag}
            >
              <span>
                <FindingTitle rule={d.finding.rule} />
                <span className={styles.msg}>{d.reason}</span>
                <span className={`${styles.mono} ${styles.file}`}>
                  {d.finding.file}
                  {d.finding.line !== null ? `:${d.finding.line}` : ""}
                </span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </span>
  );
}
