import { CircleAlert, TriangleAlert } from "lucide-react";
import { Button, Proposals, type ProposalState } from "../../ui";
import styles from "./Import.module.css";
import type { DetectionControl } from "./useDetection";

/** Review step: Claude's progress, then the proposed bundles, or why there are none. */
export function DetectionSection({
  control,
  proposals,
  names,
  count,
}: {
  control: DetectionControl;
  proposals: ProposalState;
  names: ReadonlyMap<string, string>;
  /** Artifacts Claude is reading. */
  count: number;
}) {
  const { phase } = control;
  if (phase.kind === "idle") return null;
  const noun = `${count} artifact${count === 1 ? "" : "s"}`;
  return (
    <>
      <h2 className={styles.h}>
        Proposed bundles <span>artifacts that only work together</span>
      </h2>
      {phase.kind === "running" && (
        <div className={styles.detect} role="status">
          <div className={styles.detectHead}>
            <span className={styles.spinner} aria-hidden="true" />
            <div>
              <b>Claude is reading {noun}…</b>
              <div className={styles.muted}>
                Detecting bundles and suggesting tags through your{" "}
                <span className={styles.mono}>claude</span> CLI.
                {phase.total > 0 &&
                  ` Batch ${Math.min(phase.done + 1, phase.total)} of ${phase.total}.`}
              </div>
            </div>
          </div>
          <progress
            className={styles.bar}
            aria-label="Detection progress"
            max={Math.max(phase.total, 1)}
            value={phase.done}
          />
        </div>
      )}
      {phase.kind === "missing" && (
        <div className={styles.callout} data-tone="warn">
          <TriangleAlert size={16} aria-hidden="true" />
          <div>
            <b>Claude CLI not found.</b>
            <p>
              Bundles and tags weren't suggested. Import now and add them by
              hand from each artifact, or install the CLI and re-run detection
              later.
            </p>
          </div>
        </div>
      )}
      {phase.kind === "off" && (
        <div className={styles.callout} data-tone="neutral">
          <p>
            Detection at import is off in Settings, so no bundles or tags were
            suggested.
          </p>
          <Button small onClick={control.detectNow}>
            Detect now
          </Button>
        </div>
      )}
      {phase.kind === "error" && (
        <div className={styles.callout} data-tone="bad" role="alert">
          <CircleAlert size={16} aria-hidden="true" />
          <p>
            Claude couldn't finish: {phase.message}. Import without bundles, or
            retry.
          </p>
          <Button small onClick={control.retry}>
            Retry
          </Button>
        </div>
      )}
      {phase.kind === "done" && <Proposals state={proposals} names={names} />}
    </>
  );
}
