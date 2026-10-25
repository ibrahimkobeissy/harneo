import { useEffect, useState } from "react";
import { usePersisted } from "../../ui";

/** Mirrors `--size-detail` in tokens.css: the narrowest the panel gets, and the reset width. */
export const DETAIL_MIN = 460;
/** The widest the panel gets, as a share of the window. */
const DETAIL_MAX_SHARE = 0.6;

/** The widest allowed width for a window; never below the minimum. */
export function detailMax(windowWidth: number): number {
  return Math.max(DETAIL_MIN, Math.floor(windowWidth * DETAIL_MAX_SHARE));
}

function useWindowWidth(): number {
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}

/**
 * The detail panel's remembered width, clamped to the current window. The stored value is kept
 * as chosen, so growing the window back restores it.
 */
export function useDetailWidth() {
  const max = detailMax(useWindowWidth());
  const [stored, setStored] = usePersisted(
    "harneo.detail.width",
    DETAIL_MIN,
    (raw) => {
      const n = Number(raw);
      return Number.isFinite(n) ? n : null;
    },
  );
  const width = Math.min(max, Math.max(DETAIL_MIN, Math.round(stored)));
  return { width, min: DETAIL_MIN, max, setWidth: setStored };
}
