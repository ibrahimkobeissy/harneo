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
  const max = Math.max(1, ...weekly);
  return (
    <>
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
          />
        ))}
      </div>
      <div className={styles.axis} aria-hidden="true">
        <span>8 weeks ago</span>
        <span>this week</span>
      </div>
    </>
  );
}
