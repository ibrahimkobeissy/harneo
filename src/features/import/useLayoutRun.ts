import { useCallback, useEffect, useRef, useState } from "react";
import type { LayoutStage } from "../../bindings/LayoutStage";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import {
  importLayoutSettingGet,
  libraryPreviewLayout,
  onLayoutProgress,
} from "../../lib/ipc";

export interface LayoutRun {
  /** Where the run is while Claude is working; null when it is not running. */
  stage: LayoutStage | null;
  /** Why the last run failed; null when nothing failed. */
  failure: string | null;
  /** The failure was seen and the user chose to go on with what the rules found. */
  skipped: boolean;
  /** A failure that still needs a choice: the import waits. */
  blocked: boolean;
  /** Asks Claude to place the held preview's files (also Retry). */
  ask: () => void;
  /** Goes on with what tiers 1 and 2 found. */
  skip: () => void;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Tier 3 of the layout resolver over the held preview. The user starts it with `ask`; with
 * `import.layoutWithClaude` on it also starts by itself once a preview with unplaced files
 * exists, and `force` starts it whatever the setting says (Ask Claude again). `onTicket`
 * receives the updated preview.
 */
export function useLayoutRun(
  ticket: PreviewTicket | null,
  force: boolean,
  onTicket: (next: PreviewTicket) => void,
): LayoutRun {
  const [stage, setStage] = useState<LayoutStage | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [skipped, setSkipped] = useState(false);
  const token = ticket?.token ?? null;
  const tokenRef = useRef<string | null>(token);
  const handler = useRef(onTicket);
  const unplaced = useRef(0);
  const autoFor = useRef<string | null>(null);
  useEffect(() => {
    handler.current = onTicket;
    tokenRef.current = token;
    unplaced.current = ticket?.unplaced.length ?? 0;
  });

  useEffect(() => {
    let live = true;
    let stop: (() => void) | undefined;
    void onLayoutProgress((event) => {
      if (!live || event.token !== tokenRef.current) return;
      setStage(event.stage === "done" ? null : event.stage);
    }).then((unlisten) => {
      if (live) stop = unlisten;
      else unlisten();
    });
    return () => {
      live = false;
      stop?.();
    };
  }, []);

  const ask = useCallback(() => {
    const mine = tokenRef.current;
    if (!mine) return;
    setFailure(null);
    setSkipped(false);
    setStage("reading");
    libraryPreviewLayout(mine).then(
      (next) => {
        if (tokenRef.current !== mine) return;
        setStage(null);
        handler.current(next);
      },
      (error: unknown) => {
        if (tokenRef.current !== mine) return;
        setStage(null);
        setFailure(messageOf(error));
      },
    );
  }, []);

  // A new preview: forget the last run, then start by itself when asked to.
  useEffect(() => {
    if (!token) return;
    const fresh = autoFor.current !== token;
    autoFor.current = token;
    if (fresh) {
      setFailure(null);
      setSkipped(false);
      setStage(null);
      if (force) ask();
    }
    if (force) return;
    let live = true;
    void importLayoutSettingGet().then(
      (on) => {
        if (live && on === true && unplaced.current > 0) ask();
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [token, force, ask]);

  const skip = useCallback(() => setSkipped(true), []);
  return {
    stage,
    failure,
    skipped,
    blocked: failure !== null && !skipped,
    ask,
    skip,
  };
}
