import { useCallback, useEffect, useRef, useState } from "react";
import type { Audit } from "../../bindings/Audit";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import {
  auditConfirmSettingGet,
  claudeCliStatus,
  libraryPreviewConfirm,
  libraryPreviewConfirmCancel,
  onPreviewConfirmProgress,
} from "../../lib/ipc";
import { confirmationFailed, confirmationReason } from "../../ui";

/** Key under which a failure of the whole run (not of one candidate) is kept. */
const RUN = "";

export interface PreviewConfirmation {
  /** How far Claude is through the flagged candidates; null when it is not running. */
  progress: ConfirmProgress | null;
  /** Claude is not confirming (the setting is off or the CLI is missing); null until known. */
  phase1Only: { cliMissing: boolean } | null;
  /** Why Claude could not confirm some candidate; null when nothing failed. */
  failure: { reason: string; retrying: boolean } | null;
  /** The failure was seen and the user chose to go on with phase 1's findings. */
  skipped: boolean;
  /** A failure that still needs a choice: the import waits. */
  blocked: boolean;
  /** Runs phase 2 again on the held preview. */
  retry: () => void;
  /** Goes on without Claude: the findings stay as the rules found them. */
  skip: () => void;
}

function without(
  record: Record<string, string>,
  key: string,
): Record<string, string> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key));
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Phase 2 over the held preview, in the background: once a preview exists and Claude can
 * confirm, the backend judges the flagged candidates and `onVerdict` receives each one's
 * audit as it arrives. Leaving the import, or a newer preview, stops the run. A candidate
 * Claude could not confirm is reported through `failure`, with a retry. A new `round` on the
 * same token (Claude placed more rows) runs it again; verdicts already given are reused.
 */
export function usePreviewConfirmation(
  token: string | null,
  onVerdict: (id: string, audit: Audit) => void,
  round = 0,
): PreviewConfirmation {
  const [progress, setProgress] = useState<ConfirmProgress | null>(null);
  const [phase1Only, setPhase1Only] =
    useState<PreviewConfirmation["phase1Only"]>(null);
  // Why each candidate (or the run itself) failed; empty when all is well.
  const [failures, setFailures] = useState<Record<string, string>>({});
  const [retrying, setRetrying] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const verdict = useRef(onVerdict);
  useEffect(() => {
    verdict.current = onVerdict;
  });
  const start = useRef<(() => Promise<void>) | null>(null);

  useEffect(() => {
    if (!token) return;
    let live = true;
    let stop: (() => void) | undefined;
    setFailures({});
    setSkipped(false);
    setRetrying(false);
    void (async () => {
      const on = await auditConfirmSettingGet().then(
        (v) => v !== false,
        () => true,
      );
      const found = on
        ? await claudeCliStatus().then(
            (cli) => cli?.found === true,
            () => false,
          )
        : false;
      if (!live) return;
      setPhase1Only(on && found ? null : { cliMissing: on && !found });
      if (!on || !found) return;
      const unlisten = await onPreviewConfirmProgress((event) => {
        if (!live || event.token !== token) return;
        if (event.candidate) {
          const { id, audit } = event.candidate;
          verdict.current(id, audit);
          const c = audit.confirmation;
          setFailures((prev) => {
            const rest = without(prev, id);
            return c && confirmationFailed(c)
              ? { ...rest, [id]: confirmationReason(c) }
              : rest;
          });
        }
        setProgress(
          event.done >= event.total
            ? null
            : { done: event.done, total: event.total },
        );
      });
      if (!live) {
        unlisten();
        return;
      }
      stop = unlisten;
      start.current = async () => {
        try {
          await libraryPreviewConfirm(token);
        } catch (error) {
          if (live)
            setFailures((prev) => ({ ...prev, [RUN]: messageOf(error) }));
        } finally {
          if (live) setProgress(null);
        }
      };
      await start.current();
    })();
    return () => {
      live = false;
      start.current = null;
      stop?.();
      setProgress(null);
      void libraryPreviewConfirmCancel(token).catch(() => undefined);
    };
  }, [token, round]);

  const retry = useCallback(() => {
    const run = start.current;
    if (!run) return;
    setSkipped(false);
    setRetrying(true);
    setFailures((prev) => without(prev, RUN));
    void run().finally(() => setRetrying(false));
  }, []);
  const skip = useCallback(() => setSkipped(true), []);

  const first = Object.values(failures)[0];
  const failure = first === undefined ? null : { reason: first, retrying };
  return {
    progress,
    phase1Only,
    failure,
    skipped,
    blocked: failure !== null && !skipped,
    retry,
    skip,
  };
}
