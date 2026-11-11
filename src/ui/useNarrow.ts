import { useSyncExternalStore } from "react";

/** Below this width the filter column becomes a sheet (DESIGN.md: 1180px and below). */
export const NARROW_QUERY = "(max-width: 75rem)";

function subscribe(notify: () => void): () => void {
  if (typeof window.matchMedia !== "function") return () => undefined;
  const list = window.matchMedia(NARROW_QUERY);
  list.addEventListener("change", notify);
  return () => list.removeEventListener("change", notify);
}

function snapshot(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia(NARROW_QUERY).matches
  );
}

/** Whether the window is narrow enough to hide the filter column behind a button. */
export function useNarrow(): boolean {
  return useSyncExternalStore(subscribe, snapshot, () => false);
}
