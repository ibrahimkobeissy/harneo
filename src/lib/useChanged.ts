import { useEffect, useRef } from "react";
import type { ChangeScope } from "../bindings/ChangeScope";
import { onChanged } from "./ipc";

/**
 * Calls `callback` when the live rescan reports a change on disk (the library store or a
 * registered project). Screens use it to reload; the newest `callback` is always the one called.
 */
/** Stops a listener; the event bridge may already be gone (the window closing, or a test's mocks). */
function quietly(unlisten: () => void): void {
  try {
    unlisten();
  } catch (err) {
    console.debug("harneo: listener already gone", err);
  }
}

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
        if (unmounted) quietly(unlisten);
        else stop = unlisten;
      },
      // Outside the app (no event bridge) there is nothing to listen to.
      () => {},
    );
    return () => {
      unmounted = true;
      if (stop) quietly(stop);
    };
  }, []);
}
