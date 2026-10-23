import { Activity } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import type { Facets } from "../../bindings/Facets";
import type { Status } from "../../bindings/Status";
import type { UsageSummary } from "../../bindings/UsageSummary";
import { useUsageIndex } from "../../app/usageIndex";
import { usageSummary } from "../../lib/ipc";
import { Panel } from "../../ui";
import { statusLabels } from "./params";
import styles from "./Usage.module.css";
import { SessionBars } from "./UsageBars";

const colors: Record<Status, string> = {
  current: "var(--green-fill)",
  unused: "var(--line2)",
  outdated: "var(--amber-fill)",
  findings: "var(--red-fill)",
  notUsedYet: "var(--blue)",
};

function count(facets: Facets, status: Status): number {
  return facets.statuses.find((s) => s.status === status)?.count ?? 0;
}

function Health({
  facets,
  indexed,
  selected,
  onStatus,
}: {
  facets: Facets;
  indexed: boolean;
  selected: Status[];
  onStatus: (status: Status) => void;
}) {
  // Before the first index every tracked artifact computes as unused: count those as current.
  const unusedRaw = count(facets, "unused");
  const unused = indexed ? unusedRaw : 0;
  const outdated = count(facets, "outdated");
  const findings = count(facets, "findings");
  const current =
    count(facets, "current") +
    count(facets, "notUsedYet") +
    (indexed ? 0 : unusedRaw);
  const issues = unused + outdated + findings;
  // While indexing, current and unused are not known yet: say so instead of printing guesses.
  const reading = (status: Status) =>
    !indexed && (status === "current" || status === "unused");
  const segments: { status: Status; n: number; label: string }[] = [
    { status: "current", n: current, label: "current" },
    { status: "unused", n: unused, label: "unused" },
    { status: "outdated", n: outdated, label: "outdated" },
    { status: "findings", n: findings, label: "findings" },
  ];
  return (
    <Panel
      title="Library health"
      titleId="ins-health"
      aside={
        issues > 0 && (
          <Link className={styles.review} to="/cleanup">
            Review {issues} {issues === 1 ? "issue" : "issues"}
          </Link>
        )
      }
    >
      <div
        className={styles.stack}
        role="img"
        aria-label={segments
          .map((s) =>
            reading(s.status)
              ? `${s.label} reading logs`
              : `${s.n} ${s.label === "findings" ? "with findings" : s.label}`,
          )
          .join(", ")}
      >
        {segments.map(
          (s) =>
            s.n > 0 && (
              <i
                key={s.status}
                data-status={s.status}
                style={{ flex: s.n, background: colors[s.status] }}
              />
            ),
        )}
      </div>
      <div className={styles.leg}>
        {segments.map((s) => {
          const body = (
            <>
              <i
                className={styles.dot}
                data-status={s.status}
                style={{ background: colors[s.status] }}
              />
              {reading(s.status) ? (
                <>
                  {s.label} <span className={styles.reading}>reading logs</span>
                </>
              ) : (
                <>
                  <b>{s.n}</b> {s.label}
                </>
              )}
            </>
          );
          // Current is the rest: there is no Status filter that means only "healthy".
          return s.status === "current" ? (
            <span key={s.status} className={styles.legItem}>
              {body}
            </span>
          ) : (
            <button
              key={s.status}
              type="button"
              className={styles.legItem}
              aria-pressed={selected.includes(s.status)}
              aria-label={`Filter by ${statusLabels[s.status].toLowerCase()}: ${reading(s.status) ? "reading logs" : s.n}`}
              onClick={() => onStatus(s.status)}
            >
              {body}
            </button>
          );
        })}
      </div>
    </Panel>
  );
}

function Activity8({ indexed }: { indexed: boolean }) {
  const { progress, error, runs } = useUsageIndex();
  const [summary, setSummary] = useState<UsageSummary | null>(null);
  useEffect(() => {
    let current = true;
    usageSummary().then(
      (s) => {
        if (current && s && Array.isArray(s.weekly)) setSummary(s);
      },
      () => {},
    );
    return () => {
      current = false;
    };
  }, [runs]);

  const noData = summary !== null && summary.weekly.every((w) => w === 0);
  let note: string | null = null;
  if (indexed && summary && !noData) {
    const { thisWeek, changePct } = summary;
    note = `${thisWeek} this week${
      changePct === null ? "" : ` · ${changePct >= 0 ? "+" : ""}${changePct}%`
    }`;
  }
  let body;
  if (!indexed) {
    const pct =
      progress && progress.filesTotal > 0
        ? Math.round((progress.filesDone / progress.filesTotal) * 100)
        : 0;
    body = (
      <div role="status">
        <p className={styles.empty}>
          <Activity size={16} aria-hidden="true" />
          {error ? (
            <span className={styles.err}>
              Claude&apos;s session logs could not be read: {error}
            </span>
          ) : (
            <span>
              Reading Claude&apos;s session logs…
              {progress && progress.filesTotal > 0 && (
                <span className={styles.mono}>
                  {" "}
                  {progress.filesDone} of {progress.filesTotal} files
                </span>
              )}
            </span>
          )}
        </p>
        {!error && (
          <span className={styles.progress} aria-hidden="true">
            <i style={{ width: `${pct}%` }} />
          </span>
        )}
      </div>
    );
  } else if (summary === null) {
    body = <p className={styles.empty} aria-hidden="true" />;
  } else if (noData) {
    body = (
      <p className={styles.empty}>
        <Activity size={16} aria-hidden="true" />
        <span>
          No sessions recorded yet. Counts come from Claude&apos;s session logs
          in <span className={styles.mono}>~/.claude/projects/</span>.
        </span>
      </p>
    );
  } else {
    body = (
      <SessionBars
        weekly={summary.weekly}
        label={`Sessions per week for 8 weeks, latest ${summary.thisWeek}`}
      />
    );
  }
  return (
    <Panel
      title="Sessions using library artifacts"
      titleId="ins-activity"
      aside={note && <span className={styles.headNote}>{note}</span>}
    >
      {body}
    </Panel>
  );
}

/** The library's insight row: health bar with a filtering legend, and the 8-week sessions chart. */
export function Insights({
  facets,
  selected,
  onStatus,
}: {
  facets: Facets;
  selected: Status[];
  onStatus: (status: Status) => void;
}) {
  const { indexed } = useUsageIndex();
  return (
    <div className={styles.ins}>
      <Health
        facets={facets}
        indexed={indexed}
        selected={selected}
        onStatus={onStatus}
      />
      <Activity8 indexed={indexed} />
    </div>
  );
}
