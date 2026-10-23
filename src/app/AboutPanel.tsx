import { Info } from "lucide-react";
import { Button, Panel } from "../ui";
import styles from "./AboutPanel.module.css";
import { useAppInfo } from "./useAppInfo";

/**
 * The version and data folder. `grouped` gives it the look of a Settings group
 * (header strip, hairline rows); `dataFolder={false}` leaves the folder out
 * where the screen already shows it.
 */
export function AboutPanel({
  grouped = false,
  dataFolder = true,
}: {
  grouped?: boolean;
  dataFolder?: boolean;
}) {
  const state = useAppInfo();

  if (state.status === "loading") return <p role="status">Loading…</p>;
  if (state.status === "error") {
    return (
      <div role="alert" className={styles.error}>
        <p>
          Couldn't read the version and data folder from the app:{" "}
          {state.message}
        </p>
        <Button small onClick={state.retry}>
          Try again
        </Button>
      </div>
    );
  }
  const details = (
    <>
      <dl className={styles.kv}>
        <dt>Version</dt>
        <dd className={styles.mono}>{state.info.version}</dd>
        {dataFolder && (
          <>
            <dt>Data folder</dt>
            <dd className={styles.mono}>{state.info.dataDir}</dd>
          </>
        )}
      </dl>
      {dataFolder && (
        <p className={styles.note}>
          The library, profiles, project registry and backups live in the data
          folder.
        </p>
      )}
    </>
  );
  if (!grouped) {
    return (
      <Panel title="About Harneo" titleId="about-title">
        {details}
      </Panel>
    );
  }
  return (
    <section className={styles.group} aria-labelledby="about-title">
      <header className={styles.ghead}>
        <Info size={15} aria-hidden="true" />
        <h2 id="about-title">About Harneo</h2>
      </header>
      <div className={styles.body}>{details}</div>
    </section>
  );
}
