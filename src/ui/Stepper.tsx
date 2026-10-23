import { Check } from "lucide-react";
import styles from "./Stepper.module.css";

/** A numbered progress list; the current step carries `aria-current`. */
export function Stepper({
  steps,
  at,
}: {
  steps: readonly string[];
  at: number;
}) {
  return (
    <ol className={styles.steps} aria-label="Progress">
      {steps.map((label, i) => (
        <li
          key={label}
          className={
            i === at
              ? `${styles.step} ${styles.on}`
              : i < at
                ? `${styles.step} ${styles.passed}`
                : styles.step
          }
          aria-current={i === at ? "step" : undefined}
        >
          {i > 0 && <span className={styles.stepline} aria-hidden="true" />}
          <span className={styles.num}>
            {i < at ? <Check size={13} aria-hidden="true" /> : i + 1}
          </span>
          {label}
        </li>
      ))}
    </ol>
  );
}
