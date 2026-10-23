import type { Tier } from "../bindings/Tier";
import { tierLabels } from "./audit";
import styles from "./TrustMeter.module.css";

export { tierLabels };

/**
 * The one trust meter. `sm` is a small bar and the score (tables, cards), `md` adds the tier name
 * (detail), `chip` is the compact bar-less form: tier name and score on the tier's wash (import).
 * `score` and `tier` are null for an artifact that has not been audited: it reads "Not audited".
 * Screen readers always hear the score and the tier.
 */
export function TrustMeter({
  score,
  tier,
  size = "sm",
}: {
  score: number | null;
  tier: Tier | null;
  size?: "sm" | "md" | "chip";
}) {
  if (score === null || tier === null) {
    return (
      <span className={`${styles.meter} ${styles.none}`} data-size={size}>
        Not audited
      </span>
    );
  }
  const label = tierLabels[tier];
  const title = `Trust ${score} of 100: ${label}`;
  if (size === "chip") {
    return (
      <span
        className={`${styles.meter} ${styles.chip}`}
        data-tier={tier}
        data-size={size}
        title={title}
      >
        {label}
        <span className={styles.chipScore}>
          <span className={styles.sr}>, trust </span>
          {score}
        </span>
      </span>
    );
  }
  return (
    <span
      className={styles.meter}
      data-tier={tier}
      data-size={size}
      title={title}
    >
      <span className={styles.bar} aria-hidden="true">
        <i style={{ width: `${Math.max(0, Math.min(100, score))}%` }} />
      </span>
      <span className={styles.score}>
        <span className={styles.sr}>Trust </span>
        {score}
      </span>
      {size === "md" ? (
        <span className={styles.tier}>{label}</span>
      ) : (
        <span className={styles.sr}>, {label}</span>
      )}
    </span>
  );
}
