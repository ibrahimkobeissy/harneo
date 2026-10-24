import { useEffect, useRef, useState } from "react";
import type { Audit } from "../../bindings/Audit";
import type { ConfirmProgress } from "../../bindings/ConfirmProgress";
import {
  auditConfirmSettingGet,
  claudeCliStatus,
  libraryPreviewConfirm,
  libraryPreviewConfirmCancel,
  onPreviewConfirmProgress,
} from "../../lib/ipc";

export interface PreviewConfirmation {
  /** How far Claude is through the flagged candidates; null when it is not running. */
  progress: ConfirmProgress | null;
  /** Claude is not confirming (the setting is off or the CLI is missing); null until known. */
  phase1Only: { cliMissing: boolean } | null;
}

/**
 * Phase 2 over the held preview, in the background: once a preview exists and Claude can
 * confirm, the backend judges the flagged candidates and `onVerdict` receives each one's
 * audit as it arrives. Leaving the import, or a newer preview, stops the run.
 */
export function usePreviewConfirmation(
  token: string | null,
  onVerdict: (id: string, audit: Audit) => void,
): PreviewConfirmation {
  const [progress, setProgress] = useState<ConfirmProgress | null>(null);
  const [phase1Only, setPhase1Only] =
    useState<PreviewConfirmation["phase1Only"]>(null);
  const verdict = useRef(onVerdict);
  useEffect(() => {
    verdict.current = onVerdict;
  });

  useEffect(() => {
    if (!token) return;
    let live = true;
    let stop: (() => void) | undefined;
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
          verdict.current(event.candidate.id, event.candidate.audit);
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
      try {
        await libraryPreviewConfirm(token);
      } catch {
        // Claude failing leaves the findings as phase 1 found them.
      } finally {
        if (live) setProgress(null);
      }
    })();
    return () => {
      live = false;
      stop?.();
      setProgress(null);
      void libraryPreviewConfirmCancel(token).catch(() => undefined);
    };
  }, [token]);

  return { progress, phase1Only };
}
