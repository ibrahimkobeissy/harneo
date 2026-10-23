import styles from "./Usage.module.css";

/** Sessions per week, oldest first; only the latest bar is strong. */
export function SessionBars({
  weekly,
  height = 40,
  label,
}: {
  weekly: readonly number[];
  height?: number;
  label: string;
}) {
  const peak = Math.max(0, ...weekly);
  const max = Math.max(1, peak);
  return (
    <>
      <div className={styles.chart}>
        {peak > 0 && (
          <span className={styles.max} aria-hidden="true">
            max {peak}
          </span>
        )}
        <div
          className={styles.bars}
          style={{ height }}
          role="img"
          aria-label={label}
        >
          {weekly.map((n, i) => (
            <i
              key={i}
              data-now={i === weekly.length - 1 || undefined}
              style={{ height: `${Math.max(7, Math.round((n / max) * 100))}%` }}
              title={`${n} ${n === 1 ? "session" : "sessions"}`}
            >
              {i === weekly.length - 1 && (
                <b className={styles.latest} aria-hidden="true">
                  {n}
                </b>
              )}
            </i>
          ))}
        </div>
      </div>
      <ul className={styles.weeks}>
        {weekly.map((n, i) => (
          <li key={i}>
            {i === weekly.length - 1
              ? "This week"
              : `${weekly.length - 1 - i} ${weekly.length - 1 - i === 1 ? "week" : "weeks"} ago`}
            : {n} {n === 1 ? "session" : "sessions"}
          </li>
        ))}
      </ul>
      <div className={styles.axis} aria-hidden="true">
        <span>8 weeks ago</span>
        <span>this week</span>
      </div>
    </>
  );
}
