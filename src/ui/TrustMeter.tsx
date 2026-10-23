import type { Tier } from "../bindings/Tier";
import styles from "./TrustMeter.module.css";

export const tierLabels: Record<Tier, string> = {
  trusted: "Trusted",
  review: "Review",
  risky: "Risky",
  dangerous: "Dangerous",
};

/**
 * A small bar, the score and (at size "md") the tier name, coloured by tier. `score` and
 * `tier` are null for an artifact that has not been audited: it reads "Not audited".
 */
export function TrustMeter({
  score,
  tier,
  size = "sm",
}: {
  score: number | null;
  tier: Tier | null;
  size?: "sm" | "md";
}) {
  if (score === null || tier === null) {
    return (
      <span className={`${styles.meter} ${styles.none}`} data-size={size}>
        Not audited
      </span>
    );
  }
  const label = tierLabels[tier];
  return (
    <span
      className={styles.meter}
      data-tier={tier}
      data-size={size}
      title={`Trust ${score} of 100: ${label}`}
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
