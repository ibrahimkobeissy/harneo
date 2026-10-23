import { Boxes } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import type { Detection } from "../../bindings/Detection";
import { Modal } from "../../app/Modal";
import {
  bundlesDetectLibrary,
  claudeCliStatus,
  linkAdd,
  linkReject,
  onBundlesDetectProgress,
} from "../../lib/ipc";
import { resolveNames } from "../../lib/names";
import { announce, Button, Proposals, useProposals } from "../../ui";
import { DialogFrame } from "./NewCollectionDialog";
import styles from "./FindBundles.module.css";
import { useEscape } from "./useEscape";

/** Artifacts per Claude call (core's `BATCH`). */
const BATCH = 20;

type Phase =
  | { kind: "confirm" }
  | { kind: "running"; done: number; total: number }
  | { kind: "missing" }
  | { kind: "error"; message: string }
  | { kind: "done"; detection: Detection };

const noLinks: Detection["links"] = [];

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * "Find bundles with Claude": asks Claude which of `ids` (the whole library when null) need each
 * other and shows its proposals as in the import Review step. Accepting stores them as Claude
 * links; a group switched off is remembered so a re-run does not offer it again.
 */
export function FindBundlesDialog({
  ids,
  total,
  onClose,
  onAccepted,
}: {
  ids: string[] | null;
  /** Artifacts in the library, for the pre-run step of a whole-library run. */
  total?: number;
  onClose: () => void;
  onAccepted: () => void;
}) {
  // Reading the whole library asks first; one artifact or a chosen few run straight away.
  const [phase, setPhase] = useState<Phase>(
    ids === null ? { kind: "confirm" } : { kind: "running", done: 0, total: 0 },
  );
  const [started, setStarted] = useState(ids !== null);
  const [attempt, setAttempt] = useState(0);
  const [names, setNames] = useState<ReadonlyMap<string, string>>(new Map());
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const links = phase.kind === "done" ? phase.detection.links : noLinks;
  const proposals = useProposals(links);
  const scope = useRef(ids);
  useEscape(onClose);

  useEffect(() => {
    if (!started) return;
    let live = true;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const cli = await claudeCliStatus();
        if (!live) return;
        if (!cli?.found) {
          setPhase({ kind: "missing" });
          return;
        }
        stop = await onBundlesDetectProgress((p) => {
          if (live) setPhase({ kind: "running", done: p.done, total: p.total });
        }).catch(() => undefined);
        const detection = await bundlesDetectLibrary(scope.current, true);
        if (!live) return;
        const found = await resolveNames(
          detection.links.flatMap((l) => [l.from, l.to]),
        );
        if (!live) return;
        setNames(found);
        setPhase({ kind: "done", detection });
      } catch (error) {
        if (live) setPhase({ kind: "error", message: messageOf(error) });
      } finally {
        stop?.();
      }
    })();
    return () => {
      live = false;
    };
  }, [attempt, started]);

  async function accept() {
    setSaving(true);
    setSaveError(null);
    try {
      for (const l of proposals.accepted) {
        await linkAdd(l.from, l.to, "ai", l.reason);
      }
      for (const l of proposals.rejected) {
        await linkReject(l.from, l.to).catch(() => undefined);
      }
      const n = proposals.accepted.length;
      announce(`Added ${n} link${n === 1 ? "" : "s"} found by Claude`);
      onAccepted();
      onClose();
    } catch (error) {
      setSaveError(messageOf(error));
      setSaving(false);
    }
  }

  function run() {
    setPhase({ kind: "running", done: 0, total: 0 });
    setStarted(true);
  }

  const count = proposals.accepted.length;
  const subject =
    ids === null
      ? "the whole library"
      : ids.length === 1
        ? "this artifact"
        : `${ids.length} artifacts`;
  return (
    <Modal label="Find bundles with Claude" onClose={onClose}>
      <DialogFrame
        title="Find bundles with Claude"
        sub={`Claude reads ${subject} through your claude CLI and proposes which artifacts only work together. Nothing is saved until you accept.`}
        onSubmit={(e) => {
          e.preventDefault();
          if (phase.kind === "confirm") run();
          else if (phase.kind === "done" && count > 0) void accept();
        }}
        footer={
          <>
            <Button onClick={onClose}>
              {phase.kind === "done" && count > 0 ? "Cancel" : "Close"}
            </Button>
            {phase.kind === "confirm" && (
              <Button variant="primary" type="submit">
                Run
              </Button>
            )}
            {phase.kind === "done" && (
              <Button
                variant="primary"
                type="submit"
                disabled={count === 0 || saving}
              >
                {count === 0
                  ? "Nothing to add"
                  : `Add ${count} link${count === 1 ? "" : "s"}`}
              </Button>
            )}
          </>
        }
      >
        {phase.kind === "confirm" && (
          <p className={styles.confirm}>
            {total === undefined ? (
              <>Claude will read your whole library in batches of {BATCH}.</>
            ) : (
              <>
                Claude will read {total} artifact{total === 1 ? "" : "s"} in
                about {Math.max(1, Math.ceil(total / BATCH))} batch
                {total <= BATCH ? "" : "es"}.
              </>
            )}{" "}
            Each batch is one call to your claude CLI, so a large library takes
            a while and uses your Claude plan.
          </p>
        )}
        {phase.kind === "running" && (
          <div className={styles.busy} role="status">
            <span className={styles.spinner} aria-hidden="true" />
            <div className={styles.busyText}>
              <b>Claude is reading…</b>
              {phase.total > 0 && (
                <span className={styles.muted}>
                  Batch {Math.min(phase.done + 1, phase.total)} of {phase.total}
                </span>
              )}
              <progress
                className={styles.bar}
                aria-label="Detection progress"
                max={phase.total > 0 ? phase.total : undefined}
                value={phase.total > 0 ? phase.done : undefined}
              />
            </div>
          </div>
        )}
        {phase.kind === "missing" && (
          <p role="alert" className={styles.warn}>
            <b>Claude CLI not found.</b> Install it with{" "}
            <code>npm install -g @anthropic-ai/claude-code</code>, check it in{" "}
            <Link to="/settings">Settings</Link>, then run Find bundles again.
            Until then you can link artifacts by hand from their detail panel.
          </p>
        )}
        {phase.kind === "error" && (
          <div role="alert" className={styles.fail}>
            <div>
              <p>
                <b>Claude couldn&rsquo;t finish reading.</b>
              </p>
              <p className={styles.muted}>
                Claude&rsquo;s message: {phase.message}
              </p>
              <p>
                Retry, or check the CLI in <Link to="/settings">Settings</Link>.
              </p>
            </div>
            <Button
              small
              onClick={() => {
                setPhase({ kind: "running", done: 0, total: 0 });
                setAttempt((n) => n + 1);
              }}
            >
              Retry
            </Button>
          </div>
        )}
        {phase.kind === "done" && (
          <Proposals
            state={proposals}
            names={names}
            empty="No new bundles found: these artifacts don't depend on each other."
          />
        )}
        {saveError && (
          <p role="alert" className={styles.fail}>
            {saveError}
          </p>
        )}
      </DialogFrame>
    </Modal>
  );
}

/** The toolbar's labelled "Find bundles" button for the whole library. */
export function FindBundlesButton({ onFind }: { onFind: () => void }) {
  return (
    <Button onClick={onFind}>
      <Boxes size={14} aria-hidden="true" />
      Find bundles
    </Button>
  );
}
