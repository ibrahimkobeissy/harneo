import { CircleAlert, TriangleAlert } from "lucide-react";
import { Link } from "react-router";
import { Button, Proposals, type ProposalState } from "../../ui";
import styles from "./Import.module.css";
import type { DetectionControl } from "./useDetection";

const RERUN = "Library → Find bundles";

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
            max={phase.total > 0 ? phase.total : undefined}
            value={phase.total > 0 ? phase.done : undefined}
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
              hand from each artifact. To get suggestions, install it with{" "}
              <span className={styles.mono}>
                npm install -g @anthropic-ai/claude-code
              </span>
              , check it in <Link to="/settings">Settings</Link>, then run{" "}
              {RERUN}.
            </p>
          </div>
        </div>
      )}
      {phase.kind === "off" && (
        <div className={styles.callout} data-tone="neutral">
          <p>
            Detection at import is off in <Link to="/settings">Settings</Link>,
            so no bundles or tags were suggested. Detect now, or run {RERUN}{" "}
            later.
          </p>
          <Button small onClick={control.detectNow}>
            Detect now
          </Button>
        </div>
      )}
      {phase.kind === "skipped" && (
        <div className={styles.callout} data-tone="neutral">
          <p>
            Detection was skipped, so no bundles or tags were suggested. Detect
            now, or run {RERUN} later.
          </p>
          <Button small onClick={control.detectNow}>
            Detect now
          </Button>
        </div>
      )}
      {phase.kind === "error" && (
        <div className={styles.callout} data-tone="bad" role="alert">
          <CircleAlert size={16} aria-hidden="true" />
          <div>
            <b>Claude couldn't finish reading.</b>
            <p className={styles.muted}>Claude's message: {phase.message}</p>
            <p>
              Retry, or import without bundles and run {RERUN} later. Check the
              CLI in <Link to="/settings">Settings</Link>.
            </p>
          </div>
          <Button small onClick={control.retry}>
            Retry
          </Button>
        </div>
      )}
      {phase.kind === "done" && <Proposals state={proposals} names={names} />}
    </>
  );
}
