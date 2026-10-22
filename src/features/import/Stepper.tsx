import { Check } from "lucide-react";
import { Fragment } from "react";
import styles from "./Import.module.css";

export const stepLabels = ["Source", "Choose artifacts", "Review"] as const;

/** The three-step progress list; the current step carries `aria-current`. */
export function Stepper({ at }: { at: number }) {
  return (
    <ol className={styles.steps} aria-label="Progress">
      {stepLabels.map((label, i) => (
        <Fragment key={label}>
          {i > 0 && <span className={styles.stepline} aria-hidden="true" />}
          <li
            className={
              i === at
                ? `${styles.step} ${styles.on}`
                : i < at
                  ? `${styles.step} ${styles.passed}`
                  : styles.step
            }
            aria-current={i === at ? "step" : undefined}
          >
            <span className={styles.num}>
              {i < at ? <Check size={13} aria-hidden="true" /> : i + 1}
            </span>
            {label}
          </li>
        </Fragment>
      ))}
    </ol>
  );
}
