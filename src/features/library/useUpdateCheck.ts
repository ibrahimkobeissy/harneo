import { useCallback, useEffect, useRef, useState } from "react";
import type { CheckReport } from "../../bindings/CheckReport";
import { CHECKED_EVENT, readLastCheck, runCheck } from "../../lib/updates";

/** A source that could not be looked at, by the library's id for it. */
export interface SourceProblem {
  id: string;
  reason: string;
}

/** Files a source's last check could not place, by the library's id for the source. */
export interface SourceUnplaced {
  id: string;
  paths: string[];
  overflow: number;
}

function unplacedOf(report: CheckReport): SourceUnplaced[] {
  return report.sources
    .filter((s) => (s.unplaced?.length ?? 0) + (s.unplacedOverflow ?? 0) > 0)
    .map((s) => ({
      id: s.id,
      paths: s.unplaced ?? [],
      overflow: s.unplacedOverflow ?? 0,
    }));
}

function problemsOf(report: CheckReport): SourceProblem[] {
  return report.sources.flatMap((s): SourceProblem[] => {
    if (s.result.type === "unreachable")
      return [{ id: s.id, reason: s.result.reason }];
    if (s.result.type === "missing")
      return [{ id: s.id, reason: "The source is no longer there." }];
    return [];
  });
}

/**
 * The toolbar's update check: the last-checked time, a check on demand and the sources that
 * failed. `onChecked` runs after every check, including the automatic one at start, so the list
 * can reload its badges.
 */
export function useUpdateCheck(onChecked: () => void) {
  const [lastCheck, setLastCheck] = useState<Date | null>(null);
  const [checking, setChecking] = useState(false);
  const [problems, setProblems] = useState<SourceProblem[]>([]);
  const [unplaced, setUnplaced] = useState<SourceUnplaced[]>([]);
  const [failure, setFailure] = useState<string | null>(null);
  const notify = useRef(onChecked);
  useEffect(() => {
    notify.current = onChecked;
  });

  useEffect(() => {
    let current = true;
    readLastCheck().then(
      (at) => {
        if (current) setLastCheck(at);
      },
      () => {},
    );
    function onEvent(event: Event) {
      if (!(event instanceof CustomEvent)) return;
      setProblems(problemsOf(event.detail as CheckReport));
      setUnplaced(unplacedOf(event.detail as CheckReport));
      setLastCheck(new Date());
      notify.current();
    }
    window.addEventListener(CHECKED_EVENT, onEvent);
    return () => {
      current = false;
      window.removeEventListener(CHECKED_EVENT, onEvent);
    };
  }, []);

  const check = useCallback(async () => {
    setChecking(true);
    setFailure(null);
    try {
      await runCheck();
    } catch (err) {
      setFailure(String(err));
    } finally {
      setChecking(false);
    }
  }, []);

  return {
    lastCheck,
    checking,
    problems,
    unplaced,
    failure,
    check,
    dismiss: () => {
      setProblems([]);
      setFailure(null);
    },
  };
}
