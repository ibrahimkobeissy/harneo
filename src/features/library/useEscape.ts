import { useEffect } from "react";

/** Calls `onEscape` when Esc is pressed (the Modal leaves Esc to its owner). */
export function useEscape(onEscape: () => void) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !event.defaultPrevented) onEscape();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onEscape]);
}
