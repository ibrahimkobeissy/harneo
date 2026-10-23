import { useCallback, useEffect, useRef, useState } from "react";
import type { Detection } from "../../bindings/Detection";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import {
  bundlesDetectPreview,
  claudeCliStatus,
  onBundlesDetectProgress,
  settingsGet,
} from "../../lib/ipc";

/** Where the detection of the Review step stands. */
export type DetectionPhase =
  | { kind: "idle" }
  | { kind: "running"; done: number; total: number }
  | { kind: "off" }
  | { kind: "skipped" }
  | { kind: "missing" }
  | { kind: "error"; message: string }
  | { kind: "done"; detection: Detection };

export interface DetectionControl {
  phase: DetectionPhase;
  /** Runs detection again after an error. */
  retry: () => void;
  /** Runs detection although it is off in Settings or was skipped. */
  detectNow: () => void;
  /** Gives up on a run that is going, so the import can go ahead without bundles. */
  skip: () => void;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Asks Claude for bundles and tags over `selection` once the Review step opens (`active`), unless
 * Settings turns it off. A new selection runs again; the same one keeps its answer.
 */
export function useDetection(
  active: boolean,
  preview: PreviewTicket | null,
  selection: readonly string[],
  onDetected: (detection: Detection) => void,
): DetectionControl {
  const [phase, setPhase] = useState<DetectionPhase>({ kind: "idle" });
  const [attempt, setAttempt] = useState(0);
  const [forced, setForced] = useState(false);
  const run = useRef(0);
  const doneFor = useRef("");
  const finished = useRef(false);
  const onDone = useRef(onDetected);
  useEffect(() => {
    onDone.current = onDetected;
  });
  const key = `${preview?.source ? JSON.stringify(preview.source) : ""}|${selection.join(",")}|${attempt}|${forced}`;

  useEffect(() => {
    if (!active || !preview || selection.length === 0) return;
    if (doneFor.current === key) return;
    doneFor.current = key;
    finished.current = false;
    const id = ++run.current;
    const current = () => id === run.current;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        if (!forced) {
          const enabled = await settingsGet("bundles.atImport");
          if (!current()) return;
          // Off unless the user turned it on: it sends artifact text to Claude.
          if (enabled !== true) {
            setPhase({ kind: "off" });
            return;
          }
        }
        const cli = await claudeCliStatus();
        if (!current()) return;
        if (!cli?.found) {
          setPhase({ kind: "missing" });
          return;
        }
        setPhase({ kind: "running", done: 0, total: 0 });
        stop = await onBundlesDetectProgress((p) => {
          if (current()) {
            setPhase({ kind: "running", done: p.done, total: p.total });
          }
        }).catch(() => undefined);
        const detection = await bundlesDetectPreview(
          preview,
          [...selection],
          // Detect now is the user's own action; the automatic run relies on the setting.
          forced,
        );
        if (!current()) return;
        finished.current = true;
        onDone.current(detection);
        setPhase({ kind: "done", detection });
      } catch (error) {
        if (current()) setPhase({ kind: "error", message: messageOf(error) });
      } finally {
        stop?.();
      }
    })();
  }, [active, preview, selection, key, forced]);

  // Leaving the step abandons a run that is still going; coming back starts it afresh.
  useEffect(() => {
    if (active) return;
    run.current += 1;
    if (!finished.current) doneFor.current = "";
  }, [active]);

  const retry = useCallback(() => setAttempt((n) => n + 1), []);
  const detectNow = useCallback(() => {
    setForced(true);
    setAttempt((n) => n + 1);
  }, []);
  const skip = useCallback(() => {
    run.current += 1;
    finished.current = true;
    setPhase({ kind: "skipped" });
  }, []);
  return { phase, retry, detectNow, skip };
}
