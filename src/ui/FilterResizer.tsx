import styles from "./Filters.module.css";
import { Resizer } from "./Resizer";

/** Mirrors `--size-filters` in tokens.css: the width double-click goes back to. */
export const FILTERS_DEFAULT = 228;
export const FILTERS_MIN = 180;
export const FILTERS_MAX = 420;

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
  return (
    <Resizer
      width={width}
      min={FILTERS_MIN}
      max={FILTERS_MAX}
      grow={1}
      label="Resize filters"
      className={styles.resizer}
      onWidth={onWidth}
      onDragging={onDragging}
      onReset={() => onWidth(FILTERS_DEFAULT)}
    />
  );
}
