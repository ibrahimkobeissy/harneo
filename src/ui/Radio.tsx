import type { ComponentProps } from "react";
import styles from "./Radio.module.css";

type RadioProps = Omit<ComponentProps<"input">, "type">;

/**
 * A design-system radio. The native input stays underneath, invisible but real, so arrow keys,
 * grouping by `name` and the accessible name are the browser's; the dot is drawn beside it.
 */
export function Radio({ className, ...rest }: RadioProps) {
  return (
    <span className={[styles.radio, className].filter(Boolean).join(" ")}>
      <input type="radio" className={styles.input} {...rest} />
      <span className={styles.mark} aria-hidden="true" />
    </span>
  );
}
