import { useState } from "react";
import styles from "./MapCanvas.module.css";

/** The beat is 2 s; every dot starts at the same point of the page's clock, so they beat together. */
const BEAT_MS = 2000;

function beatDelay(): string {
  return `-${Math.round(performance.now() % BEAT_MS)}ms`;
}

/**
 * The heartbeat dot of a node: slate, one shared 2 s beat (a CSS animation, no timers),
 * blue and still while its tooltip is open. Not a tab stop: `i` on the node opens it.
 */
export function Heartbeat({
  name,
  x,
  y,
  open,
  onToggle,
}: {
  name: string;
  x: number;
  y: number;
  open: boolean;
  onToggle: () => void;
}) {
  const [delay] = useState(beatDelay);
  return (
    <button
      type="button"
      className={styles.hb}
      data-open={open}
      tabIndex={-1}
      aria-label={`How ${name} takes part in the flow`}
      aria-expanded={open}
      style={{ left: x, top: y }}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <i style={{ animationDelay: delay }} />
    </button>
  );
}
