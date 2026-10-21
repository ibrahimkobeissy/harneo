import { Button, Panel } from "../ui";
import styles from "./AboutPanel.module.css";
import { useAppInfo } from "./useAppInfo";

export function AboutPanel() {
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
  return (
    <Panel title="About Harneo" titleId="about-title">
      <dl className={styles.kv}>
        <dt>Version</dt>
        <dd className={styles.mono}>{state.info.version}</dd>
        <dt>Data folder</dt>
        <dd className={styles.mono}>{state.info.dataDir}</dd>
      </dl>
      <p className={styles.note}>
        The library, profiles, project registry and backups live in the data
        folder.
      </p>
    </Panel>
  );
}
