import { useCallback, useEffect, useState } from "react";

const STORE = "harnmap.panelHidden";

function read(): boolean {
  try {
    return sessionStorage.getItem(STORE) === "1";
  } catch {
    return false;
  }
}

function write(hidden: boolean) {
  try {
    sessionStorage.setItem(STORE, hidden ? "1" : "0");
  } catch {
    // Storage can be blocked; the choice then lasts as long as the screen.
  }
}

/** True while a field that types text has focus: the shortcut must not fire there. */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
  );
}

/**
 * Whether the side panel is collapsed, remembered for the session. `]` toggles it.
 * `reveal` names what the panel shows on request (the selected node, the history, or null): a new one
 * opens the panel again.
 */
export function usePanelHidden(
  enabled: boolean,
  reveal: string | null,
): [boolean, (hidden: boolean) => void] {
  const [hidden, setHidden] = useState(read);
  const set = useCallback((next: boolean) => {
    setHidden(next);
    write(next);
  }, []);

  // Adjusting state while rendering: a new thing to show re-opens the panel.
  const [shown, setShown] = useState(reveal);
  if (shown !== reveal) {
    setShown(reveal);
    if (reveal !== null && hidden) set(false);
  }

  useEffect(() => {
    if (!enabled) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key !== "]" || ev.ctrlKey || ev.metaKey || ev.altKey) return;
      if (typing(ev.target)) return;
      set(!hidden);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [enabled, hidden, set]);

  return [hidden, set];
}
