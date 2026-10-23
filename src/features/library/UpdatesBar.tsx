import { CircleArrowUp, RefreshCw, TriangleAlert, X } from "lucide-react";
import type { Facets } from "../../bindings/Facets";
import { Button } from "../../ui";
import styles from "./Updates.module.css";
import type { useUpdateCheck } from "./useUpdateCheck";

type Check = ReturnType<typeof useUpdateCheck>;

const stamp = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  hour: "numeric",
  minute: "2-digit",
});

/** Check for updates, when it last ran, and Update all while something is outdated. */
export function UpdatesControls({
  check,
  outdated,
  onUpdateAll,
}: {
  check: Check;
  outdated: number;
  onUpdateAll: () => void;
}) {
  return (
    <span className={styles.check}>
      <span aria-live="polite">
        {check.checking
          ? "Checking sources…"
          : check.lastCheck
            ? `Checked ${stamp.format(check.lastCheck)}`
            : "Never checked"}
      </span>
      <Button
        small
        onClick={() => void check.check()}
        disabled={check.checking}
        title="Look at every source for changed artifacts"
      >
        <RefreshCw
          size={14}
          aria-hidden="true"
          className={check.checking ? styles.spin : undefined}
        />
        Check for updates
      </Button>
      {outdated > 0 && (
        <Button small onClick={onUpdateAll}>
          <CircleArrowUp size={14} aria-hidden="true" />
          Update all ({outdated})
        </Button>
      )}
    </span>
  );
}

/** The sources that could not be looked at, in a callout the user can dismiss. */
export function UpdateProblems({
  check,
  sources,
}: {
  check: Check;
  sources: Facets["sources"];
}) {
  if (check.problems.length === 0 && check.failure === null) return null;
  const label = (id: string) =>
    sources.find((s) => s.source.id === id)?.source.label ?? id;
  return (
    <div className={`${styles.callout} ${styles.checkBar}`} data-tone="warn">
      <TriangleAlert size={16} aria-hidden="true" />
      <div className={styles.calloutBody} role="alert">
        <b>
          {check.failure !== null
            ? "The check could not run"
            : "Some sources could not be checked"}
        </b>
        {check.failure !== null && <p>{check.failure}</p>}
        {check.problems.length > 0 && (
          <ul>
            {check.problems.map((p) => (
              <li key={p.id}>
                {label(p.id)}: {p.reason}
              </li>
            ))}
          </ul>
        )}
      </div>
      <Button
        small
        variant="ghost"
        aria-label="Dismiss the update errors"
        onClick={check.dismiss}
      >
        <X size={14} aria-hidden="true" />
      </Button>
    </div>
  );
}
