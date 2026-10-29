import {
  CircleArrowUp,
  FileQuestion,
  RefreshCw,
  TriangleAlert,
  X,
} from "lucide-react";
import { useId, useState } from "react";
import type { Facets } from "../../bindings/Facets";
import { Button } from "../../ui";
import styles from "./Updates.module.css";
import type { SourceUnplaced, useUpdateCheck } from "./useUpdateCheck";

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

function UnplacedNotice({
  entry,
  label,
  onAsk,
}: {
  entry: SourceUnplaced;
  label: string;
  onAsk: () => void;
}) {
  const [open, setOpen] = useState(false);
  const listId = useId();
  const count = entry.paths.length + entry.overflow;
  return (
    <div className={`${styles.callout} ${styles.checkBar}`} data-tone="neutral">
      <FileQuestion size={16} aria-hidden="true" />
      <div className={styles.calloutBody}>
        <p>
          <span>{`${count} file${count === 1 ? "" : "s"} Harneo couldn't place`}</span>
          <span>{` in ${label}`}</span>
        </p>
        <button
          type="button"
          className={styles.linkButton}
          aria-expanded={open}
          aria-controls={listId}
          onClick={() => setOpen(!open)}
        >
          {open ? "Hide the paths" : "Show the paths"}
        </button>
        {open && (
          <ul id={listId} className={styles.mono}>
            {entry.paths.map((path) => (
              <li key={path}>{path}</li>
            ))}
            {entry.overflow > 0 && (
              <li
                className={styles.plainItem}
              >{`and ${entry.overflow} more`}</li>
            )}
          </ul>
        )}
      </div>
      <Button small onClick={onAsk}>
        Ask Claude what&apos;s in this repo
      </Button>
    </div>
  );
}

/** Per source, the files the last check could not place. Nothing is sent until Ask Claude. */
export function UnplacedNotices({
  check,
  sources,
  onAsk,
}: {
  check: Check;
  sources: Facets["sources"];
  onAsk: (sourceId: string) => void;
}) {
  return (
    <>
      {check.unplaced.map((entry) => (
        <UnplacedNotice
          key={entry.id}
          entry={entry}
          label={
            sources.find((s) => s.source.id === entry.id)?.source.label ??
            entry.id
          }
          onAsk={() => onAsk(entry.id)}
        />
      ))}
    </>
  );
}
