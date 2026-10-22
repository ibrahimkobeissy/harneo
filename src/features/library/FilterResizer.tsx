import type { KeyboardEvent, PointerEvent } from "react";
import { useRef } from "react";
import styles from "./Library.module.css";

/** Mirrors `--size-filters` in tokens.css: the width double-click goes back to. */
export const FILTERS_DEFAULT = 228;
export const FILTERS_MIN = 180;
export const FILTERS_MAX = 420;
const STEP = 16;

export function clampWidth(width: number): number {
  return Math.min(FILTERS_MAX, Math.max(FILTERS_MIN, Math.round(width)));
}

/** The drag handle on the filter column's right edge: a vertical separator you can also focus. */
export function FilterResizer({
  width,
  onWidth,
  onDragging,
}: {
  width: number;
  onWidth: (width: number) => void;
  onDragging: (dragging: boolean) => void;
}) {
  const start = useRef<{ x: number; width: number } | null>(null);

  function onKeyDown(ev: KeyboardEvent<HTMLDivElement>) {
    const next =
      ev.key === "ArrowLeft"
        ? width - STEP
        : ev.key === "ArrowRight"
          ? width + STEP
          : ev.key === "Home"
            ? FILTERS_MIN
            : ev.key === "End"
              ? FILTERS_MAX
              : null;
    if (next === null) return;
    ev.preventDefault();
    onWidth(clampWidth(next));
  }

  function onPointerDown(ev: PointerEvent<HTMLDivElement>) {
    if (ev.button !== 0) return;
    ev.preventDefault();
    ev.currentTarget.setPointerCapture(ev.pointerId);
    start.current = { x: ev.clientX, width };
    onDragging(true);
  }

  function onPointerMove(ev: PointerEvent<HTMLDivElement>) {
    if (!start.current) return;
    onWidth(clampWidth(start.current.width + ev.clientX - start.current.x));
  }

  function end(ev: PointerEvent<HTMLDivElement>) {
    if (!start.current) return;
    start.current = null;
    if (ev.currentTarget.hasPointerCapture(ev.pointerId)) {
      ev.currentTarget.releasePointerCapture(ev.pointerId);
    }
    onDragging(false);
  }

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize filters"
      aria-valuenow={width}
      aria-valuemin={FILTERS_MIN}
      aria-valuemax={FILTERS_MAX}
      tabIndex={0}
      className={styles.resizer}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      onDoubleClick={() => onWidth(FILTERS_DEFAULT)}
    />
  );
}
