import { useEffect, useRef, type ComponentProps } from "react";
import styles from "./Checkbox.module.css";

interface CheckboxProps extends Omit<ComponentProps<"input">, "type" | "ref"> {
  /** The mixed state: some, not all, of what the box stands for is on. Shown as a dash. */
  indeterminate?: boolean;
}

/**
 * A design-system checkbox. The native input stays underneath, invisible but real, so the
 * keyboard, the accessible name and form behaviour are the browser's; the box is drawn beside it.
 * Put it inside a `<label>` (or give it `aria-label`): the label then clicks it.
 */
export function Checkbox({
  indeterminate = false,
  className,
  ...rest
}: CheckboxProps) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (input.current) input.current.indeterminate = indeterminate;
  }, [indeterminate, rest.checked]);
  return (
    <span className={[styles.box, className].filter(Boolean).join(" ")}>
      <input ref={input} type="checkbox" className={styles.input} {...rest} />
      <span className={styles.mark} aria-hidden="true" />
    </span>
  );
}
