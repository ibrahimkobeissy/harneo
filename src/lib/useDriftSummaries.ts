import { useCallback, useEffect, useState } from "react";
import type { DriftSummary } from "../bindings/DriftSummary";
import { driftCheckAll } from "./ipc";
import { useChanged } from "./useChanged";

/**
 * Every project's drift summary by project id. Reloads when the live rescan reports a change;
 * a failed load just means no drift is shown.
 */
export function useDriftSummaries(): ReadonlyMap<bigint, DriftSummary> {
  const [summaries, setSummaries] = useState<ReadonlyMap<bigint, DriftSummary>>(
    new Map(),
  );
  const [attempt, setAttempt] = useState(0);
  useChanged(useCallback(() => setAttempt((n) => n + 1), []));
  useEffect(() => {
    let live = true;
    driftCheckAll().then(
      (list) => {
        if (live && Array.isArray(list)) {
          setSummaries(new Map(list.map((p) => [p.projectId, p.summary])));
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [attempt]);
  return summaries;
}
