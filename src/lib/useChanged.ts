import { useEffect, useRef } from "react";
import type { ChangeScope } from "../bindings/ChangeScope";
import { onChanged } from "./ipc";

/**
 * Calls `callback` when the live rescan reports a change on disk (the library store or a
 * registered project). Screens use it to reload; the newest `callback` is always the one called.
 */
export function useChanged(callback: (scope: ChangeScope) => void): void {
  const latest = useRef(callback);
  useEffect(() => {
    latest.current = callback;
  });
  useEffect(() => {
    let stop: (() => void) | null = null;
    let unmounted = false;
    onChanged((scope) => latest.current(scope)).then(
      (unlisten) => {
        if (unmounted) unlisten();
        else stop = unlisten;
      },
      // Outside the app (no event bridge) there is nothing to listen to.
      () => {},
    );
    return () => {
      unmounted = true;
      stop?.();
    };
  }, []);
}
