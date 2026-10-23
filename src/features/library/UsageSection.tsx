import { useEffect, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Usage } from "../../bindings/Usage";
import { useUsageIndex } from "../../app/usageIndex";
import { usageOf } from "../../lib/ipc";
import styles from "./Detail.module.css";
import { kinds } from "./kinds";
import ustyles from "./Usage.module.css";
import { isTracked, plural, relativeDay } from "./usage";
import { SessionBars } from "./UsageBars";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; usage: Usage };

/**
 * The detail panel's Usage section: sessions in 8 weeks, last used, 8 bars and the projects it
 * was used in; or why there is nothing to show for kinds the logs do not record.
 */
export function UsageSection({
  kind,
  name,
}: {
  kind: ArtifactKind;
  name: string;
}) {
  const tracked = isTracked(kind);
  const { runs } = useUsageIndex();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  useEffect(() => {
    if (!tracked) return;
    let current = true;
    usageOf(kind, name).then(
      (usage) => {
        // An unanswered call (no backend) stays "reading".
        if (current && usage && Array.isArray(usage.weekly))
          setLoad({ status: "ready", usage });
      },
      (err: unknown) => {
        if (current) setLoad({ status: "error", message: String(err) });
      },
    );
    return () => {
      current = false;
    };
  }, [tracked, kind, name, runs]);

  if (!tracked) {
    return (
      <section className={styles.sec} aria-labelledby="d-usage">
        <h3 id="d-usage">Usage</h3>
        <p className={styles.muted}>
          Not tracked: Claude&apos;s logs don&apos;t record when{" "}
          {kinds[kind].plural.toLowerCase()} are used.
        </p>
      </section>
    );
  }
  return (
    <section className={styles.sec} aria-labelledby="d-usage">
      <h3 id="d-usage">Usage</h3>
      {load.status === "loading" && (
        <p className={styles.muted}>Reading usage…</p>
      )}
      {load.status === "error" && (
        <p role="alert" className={styles.muted}>
          {load.message}
        </p>
      )}
      {load.status === "ready" && (
        <>
          <p className={styles.muted}>
            {plural(load.usage.sessions8w, "session")} in 8 weeks
            {load.usage.lastUsed
              ? ` · last ${relativeDay(load.usage.lastUsed)}`
              : " · never used"}
          </p>
          <div style={{ marginTop: "var(--space-sm)" }}>
            <SessionBars
              weekly={load.usage.weekly}
              height={48}
              label={`Sessions per week for 8 weeks, ${load.usage.sessions8w} in total`}
            />
          </div>
          {load.usage.projects.length > 0 && (
            <>
              <p className={ustyles.sub}>
                Used in {plural(load.usage.projects.length, "project")}
              </p>
              <ul className={ustyles.projects}>
                {load.usage.projects.map((p) => (
                  <li key={p} title={p}>
                    {p}
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
    </section>
  );
}
