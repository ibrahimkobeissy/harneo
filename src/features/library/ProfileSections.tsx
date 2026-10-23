import { Layers } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import type { ProfileUse } from "../../bindings/ProfileUse";
import { artifactProfiles, artifactSetManualOnly } from "../../lib/ipc";
import { announce } from "../../ui";
import detail from "./Detail.module.css";
import styles from "./ProfileSections.module.css";

/**
 * The skill's library default for manual-only invocation (its own `disable-model-invocation`
 * frontmatter counts too). Only the index changes, never the library file.
 */
export function ManualOnlySection({
  id,
  name,
  manualOnly,
  onChanged,
}: {
  id: string;
  name: string;
  manualOnly: boolean;
  onChanged: () => void;
}) {
  const [on, setOn] = useState(manualOnly);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Follow the stored value when it changes under us (the panel reloaded).
  const [seen, setSeen] = useState(manualOnly);
  if (seen !== manualOnly) {
    setSeen(manualOnly);
    setOn(manualOnly);
  }

  async function toggle() {
    const next = !on;
    setBusy(true);
    setError(null);
    try {
      await artifactSetManualOnly(id, next);
      setOn(next);
      const text = next
        ? `${name} is now manual only`
        : `${name} can be auto-invoked again`;
      setSaid(text);
      announce(text);
      onChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className={detail.sec} aria-labelledby="d-manual">
      <div className={styles.switchRow}>
        <div className={styles.switchText}>
          <h3 id="d-manual">Manual only</h3>
          <p>
            Never auto-invoked. Sets{" "}
            <span className={styles.mono}>disable-model-invocation: true</span>.
          </p>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={on}
          aria-labelledby="d-manual"
          className={styles.switch}
          disabled={busy}
          onClick={() => void toggle()}
        >
          <i />
        </button>
      </div>
      {said && (
        <p role="status" className={styles.status}>
          {said}
        </p>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </section>
  );
}

/** The profiles whose current version installs the artifact, each linking to its builder. */
export function UsedInSection({
  id,
  rev,
}: {
  id: string;
  /** Changes when a profile may have changed (the Add to… menu saved one). */
  rev: number;
}) {
  const [state, setState] = useState<{
    id: string;
    profiles: ProfileUse[] | null;
    error?: string;
  }>({ id: "", profiles: null });
  useEffect(() => {
    let current = true;
    artifactProfiles(id).then(
      (profiles) => {
        if (current)
          setState({ id, profiles: Array.isArray(profiles) ? profiles : [] });
      },
      (err: unknown) => {
        if (current) setState({ id, profiles: null, error: String(err) });
      },
    );
    return () => {
      current = false;
    };
  }, [id, rev]);
  const profiles = state.id === id ? state.profiles : null;
  return (
    <section className={detail.sec} aria-labelledby="d-used">
      <h3 id="d-used">Used in</h3>
      {state.id === id && state.error && (
        <p role="alert" className={styles.error}>
          {state.error}
        </p>
      )}
      {profiles !== null && profiles.length === 0 && (
        <p className={styles.muted}>Not in any profile yet.</p>
      )}
      {profiles !== null && profiles.length > 0 && (
        <ul className={styles.pins}>
          {profiles.map((p) => (
            <li key={String(p.id)}>
              <Link className={styles.pin} to={`/profiles/${p.id}`}>
                <Layers size={14} aria-hidden="true" />
                <span>{p.name}</span>
                <span className={styles.version}>v{p.version}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
