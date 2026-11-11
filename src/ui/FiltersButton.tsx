import { SlidersHorizontal } from "lucide-react";
import type { Ref } from "react";
import styles from "./Filters.module.css";

/** Opens the filter column when it is hidden or shown as a sheet; shows how many filters are set. */
export function FiltersButton({
  ref,
  active,
  sheet,
  open,
  onClick,
}: {
  ref?: Ref<HTMLButtonElement>;
  /** How many filters are set. */
  active: number;
  /** True when the button opens the sheet (a dialog) rather than the docked column. */
  sheet: boolean;
  open: boolean;
  onClick: () => void;
}) {
  return (
    <button
      ref={ref}
      type="button"
      className={styles.filtersBtn}
      aria-haspopup={sheet ? "dialog" : undefined}
      aria-expanded={sheet ? open : false}
      onClick={onClick}
    >
      <SlidersHorizontal size={14} aria-hidden="true" />
      Filters
      {active > 0 && <span className={styles.filtersCount}>{active}</span>}
    </button>
  );
}
