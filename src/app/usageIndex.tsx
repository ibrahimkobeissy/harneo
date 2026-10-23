import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { IndexProgress } from "../bindings/IndexProgress";
import {
  onUsageIndexProgress,
  settingsGet,
  settingsSet,
  usageIndex,
} from "../lib/ipc";

/** Settings key written after each successful index run (an ISO timestamp). */
export const LAST_INDEXED_KEY = "usage.lastIndexedAt";

/** How often the index is refreshed while the app is open. */
export const REINDEX_MS = 10 * 60 * 1000;

export interface UsageIndexState {
  /** True once an index run has completed, now or in an earlier session. */
  indexed: boolean;
  /** Set while a run is reading the logs. */
  progress: IndexProgress | null;
  /** The last run's error, cleared by the next success. */
  error: string | null;
  /** Counts completed runs: screens reload their usage numbers when it changes. */
  runs: number;
  /** Starts a run now (ignored while one is running). */
  refresh: () => void;
}

const idle: UsageIndexState = {
  indexed: true,
  progress: null,
  error: null,
  runs: 0,
  refresh: () => {},
};

const Context = createContext<UsageIndexState>(idle);

/** The usage index state; outside a provider, "indexed" with nothing running. */
export function useUsageIndex(): UsageIndexState {
  return useContext(Context);
}

/**
 * Reads Claude's session logs at start and every 10 minutes while the app is open, and tells
 * screens whether an index has ever completed: until then every tracked artifact computes as
 * unused, which the library must not show.
 */
export function UsageIndexProvider({ children }: { children: ReactNode }) {
  const [indexed, setIndexed] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [runs, setRuns] = useState(0);
  const running = useRef(false);
  const alive = useRef(true);

  const run = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    setProgress({ filesDone: 0, filesTotal: 0, newUses: 0 });
    try {
      await usageIndex();
      try {
        await settingsSet(LAST_INDEXED_KEY, new Date().toISOString());
      } catch {
        // Only the "has run once" memory is lost; the run itself worked.
      }
      if (!alive.current) return;
      setIndexed(true);
      setError(null);
      setRuns((n) => n + 1);
    } catch (err) {
      if (alive.current) setError(String(err));
    } finally {
      running.current = false;
      if (alive.current) setProgress(null);
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    let unlisten: (() => void) | undefined;
    onUsageIndexProgress((p) => {
      if (alive.current && running.current) setProgress(p);
    }).then(
      (fn) => {
        if (alive.current) unlisten = fn;
        else fn?.();
      },
      () => {},
    );
    settingsGet(LAST_INDEXED_KEY).then(
      (value) => {
        if (alive.current && value) setIndexed(true);
      },
      () => {},
    );
    const first = setTimeout(() => void run(), 0);
    const timer = setInterval(() => void run(), REINDEX_MS);
    return () => {
      alive.current = false;
      clearTimeout(first);
      clearInterval(timer);
      if (typeof unlisten === "function") unlisten();
    };
  }, [run]);

  const value = useMemo(
    () => ({ indexed, progress, error, runs, refresh: () => void run() }),
    [indexed, progress, error, runs, run],
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
