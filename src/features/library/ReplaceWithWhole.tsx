import { Replace } from "lucide-react";
import { useState } from "react";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { ReplaceOutcome } from "../../bindings/ReplaceOutcome";
import { Modal } from "../../app/Modal";
import { libraryReplacePlan, libraryReplaceWithWhole } from "../../lib/ipc";
import { Button, Callout, announce } from "../../ui";
import styles from "./Detail.module.css";

/** The plugin an unpacked part came from, by name. */
function pluginName(detail: ArtifactDetail): string {
  const from = detail.artifact.metadata.fromPlugin;
  return typeof from === "object" && from !== null && "name" in from
    ? String(from.name)
    : "its plugin";
}

/**
 * The notice of a part whose plugin must now stay whole (ADR-006), with the way back to the
 * whole plugin: the parts leave, the plugin comes in, and profiles holding the parts get it.
 */
export function WholeNotice({
  detail,
  onReplaced,
}: {
  detail: ArtifactDetail;
  /** The plugin replaced the parts: its id. */
  onReplaced: (pluginId: string) => void;
}) {
  const [asked, setAsked] = useState<ReplaceOutcome | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reason = detail.mustStayWhole;
  if (reason === undefined) return null;
  const name = pluginName(detail);

  // The profiles may have changed since the panel opened: the confirm reads them again.
  async function ask() {
    setError(null);
    try {
      setAsked(await libraryReplacePlan(detail.id));
    } catch (err) {
      setError(String(err));
    }
  }

  async function replace() {
    setBusy(true);
    setError(null);
    try {
      const done = await libraryReplaceWithWhole(detail.id);
      setAsked(null);
      setBusy(false);
      announce(`Replaced the parts of ${name} with the whole plugin`);
      onReplaced(done.pluginId);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <section className={styles.sec} aria-label="Stays whole">
      <Callout tone="warn" icon={Replace}>
        <b>
          Taken apart from {name}, which only works whole: {reason}
        </b>
        <p>
          <Button small onClick={() => void ask()}>
            Replace with the whole plugin
          </Button>
        </p>
        {asked === null && error !== null && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </Callout>
      {asked !== null && (
        <Modal label={`Replace the parts of ${name}`}>
          <div className={styles.confirm}>
            <h2>Replace the parts of {name} with the whole plugin?</h2>
            <p>
              Leaves the library: {asked.parts.map((p) => p.name).join(", ")}.
            </p>
            <p>
              {asked.profiles.length === 0
                ? "No profile holds them."
                : `Profiles that change: ${asked.profiles
                    .map((p) => `${p.name} v${p.from} → v${p.to}`)
                    .join(", ")}.`}
            </p>
            {asked.placementsDropped > 0 && (
              <p>
                They leave the map of those profiles; the map shows out of date
                until you draw it again.
              </p>
            )}
            <p>The plugin keeps their tags and collections.</p>
            {error !== null && (
              <p role="alert" className={styles.error}>
                {error}
              </p>
            )}
            <div className={styles.confirmActions}>
              <Button onClick={() => setAsked(null)} disabled={busy}>
                Cancel
              </Button>
              <Button
                variant="primary"
                onClick={() => void replace()}
                disabled={busy}
              >
                Replace
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </section>
  );
}
