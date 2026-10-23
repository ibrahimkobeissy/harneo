import type { Tier } from "../../bindings/Tier";
import styles from "./Security.module.css";

/** A small bar and the score; the colour follows the tier, the number carries the meaning. */
export function TrustMeter({ score, tier }: { score: number; tier: Tier }) {
  return (
    <span className={styles.meterWrap} data-tier={tier}>
      <span className={styles.meter} aria-hidden="true">
        <i style={{ width: `${score}%` }} />
      </span>
      <span className={styles.score}>{score}</span>
    </span>
  );
}
