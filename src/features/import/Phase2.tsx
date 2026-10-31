import { ChevronDown, ShieldQuestion } from "lucide-react";
import { useState } from "react";
import type { Candidate } from "../../bindings/Candidate";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import { Button, Callout, FindingTitle } from "../../ui";
import type { PreviewConfirmation } from "./usePreviewConfirmation";
import styles from "./Import.module.css";

/** "Confirming N of M with Claude…" while the background run is going. */
export function ConfirmingNote({
  progress,
  hint,
}: {
  progress: ConfirmProgress | null;
  /** What waits meanwhile, after the count. */
  hint?: string;
}) {
  if (progress === null) return null;
  return (
    <div className={styles.confirming} role="status">
      <span className={styles.spinner} aria-hidden="true" />
      <span>
        <b>{`Confirming ${Math.min(progress.done + 1, progress.total)} of ${progress.total} with Claude…`}</b>{" "}
        {hint && <span className={styles.muted}>{hint}</span>}
      </span>
    </div>
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

/**
 * Phase 2 could not confirm some findings. Until the user picks Retry, Continue without
 * Claude or Cancel import, the import waits. After Continue without Claude it stays as a note.
 */
export function ConfirmFailureCallout({
  phase2,
  onCancel,
}: {
  phase2: PreviewConfirmation;
  onCancel: () => void;
}) {
  const { failure } = phase2;
  if (!failure) return null;
  const lead = `Claude could not confirm the findings: ${failure.reason}.`;
  const tail = "Findings are shown as the rules found them.";
  if (phase2.skipped) {
    return (
      <Callout tone="warn" icon={ShieldQuestion}>
        <p className={styles.noMargin}>{`${lead} ${tail}`}</p>
      </Callout>
    );
  }
  return (
    <Callout tone="bad" icon={ShieldQuestion}>
      <p className={styles.noMargin}>{`${lead} ${tail}`}</p>
      <div className={styles.failureActions}>
        <Button small disabled={failure.retrying} onClick={phase2.retry}>
          {failure.retrying ? "Retrying…" : "Retry"}
        </Button>
        <Button small onClick={phase2.skip}>
          Continue without Claude
        </Button>
        <Button small variant="ghost" onClick={onCancel}>
          Cancel import
        </Button>
      </div>
    </Callout>
  );
}
