import type { KeyboardEvent, PointerEvent } from "react";
import { useRef } from "react";

const STEP = 16;

/**
 * A drag handle that resizes a panel: a focusable separator driven by the pointer or the
 * arrow keys (Home and End jump to the limits). `grow` is the direction that widens the
 * panel: 1 when dragging or pressing right makes it wider, -1 when left does.
 */
export function Resizer({
  width,
  min,
  max,
  grow,
  label,
  className,
  onWidth,
  onDragging,
  onReset,
}: {
  width: number;
  min: number;
  max: number;
  grow: 1 | -1;
  label: string;
  className?: string;
  onWidth: (width: number) => void;
  onDragging: (dragging: boolean) => void;
  onReset?: () => void;
}) {
  const start = useRef<{ x: number; width: number } | null>(null);
  const clamp = (w: number) => Math.min(max, Math.max(min, Math.round(w)));

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const wider = grow === 1 ? "ArrowRight" : "ArrowLeft";
    const narrower = grow === 1 ? "ArrowLeft" : "ArrowRight";
    const pressed = event.key;
    const next =
      pressed === wider
        ? width + STEP
        : pressed === narrower
          ? width - STEP
          : pressed === "Home"
            ? min
            : pressed === "End"
              ? max
              : null;
    if (next === null) return;
    event.preventDefault();
    onWidth(clamp(next));
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    start.current = { x: event.clientX, width };
    onDragging(true);
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (!start.current) return;
    onWidth(
      clamp(start.current.width + grow * (event.clientX - start.current.x)),
    );
  }

  function end(event: PointerEvent<HTMLDivElement>) {
    if (!start.current) return;
    start.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    onDragging(false);
  }

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className={className}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      onDoubleClick={onReset}
    />
  );
}
