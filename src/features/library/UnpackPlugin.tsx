import { PackageOpen } from "lucide-react";
import { useEffect, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { UnpackOutcome } from "../../bindings/UnpackOutcome";
import { Modal } from "../../app/Modal";
import { libraryUnpackPlan, libraryUnpackPlugin } from "../../lib/ipc";
import { Button, announce } from "../../ui";
import styles from "./Detail.module.css";

/**
 * What unpacking the plugin `id` would do, read again when `tick` changes. `null` for any other
 * kind of artifact, and while unknown.
 */
export function useUnpackPlan(
  id: string,
  kind: ArtifactKind | null,
  tick: number,
): UnpackOutcome | null {
  const [state, setState] = useState<{
    id: string;
    plan: UnpackOutcome | null;
  }>({ id: "", plan: null });
  useEffect(() => {
    if (kind !== "plugin") return;
    let current = true;
    libraryUnpackPlan(id).then(
      (plan) => {
        if (current) setState({ id, plan: plan ?? null });
      },
      () => {
        if (current) setState({ id, plan: null });
      },
    );
    return () => {
      current = false;
    };
  }, [id, kind, tick]);
  return kind === "plugin" && state.id === id ? state.plan : null;
}

/** The reason a plugin has no Unpack button. */
export function StaysWhole({ plan }: { plan: UnpackOutcome | null }) {
  if (plan === null || plan.reason === null) return null;
  return (
    <section className={styles.sec} aria-label="Stays whole">
      <p className={styles.muted}>Stays whole: {plan.reason}</p>
    </section>
  );
}

/** The Unpack action of a whole plugin, with its confirm. */
export function UnpackButton({
  id,
  name,
  plan,
  onUnpacked,
}: {
  id: string;
  name: string;
  plan: UnpackOutcome | null;
  /** The plugin became its parts: the id of the first one, or null when there is none. */
  onUnpacked: (firstPart: string | null) => void;
}) {
  const [asked, setAsked] = useState<UnpackOutcome | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (plan === null || plan.refused !== null) return null;

  // The profiles may have changed since the panel opened: the confirm reads them again.
  async function ask() {
    setError(null);
    try {
      const fresh = await libraryUnpackPlan(id);
      if (fresh.refused === null) setAsked(fresh);
      else setError(fresh.reason);
    } catch (err) {
      setError(String(err));
    }
  }

  async function unpack() {
    setBusy(true);
    setError(null);
    try {
      const done = await libraryUnpackPlugin(id);
      if (done.refused !== null) {
        setError(done.reason);
        setBusy(false);
        return;
      }
      setAsked(null);
      setBusy(false);
      announce(`Unpacked ${name} into ${done.parts.length} artifacts`);
      onUnpacked(done.parts[0]?.id ?? null);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <>
      <Button small onClick={() => void ask()}>
        <PackageOpen size={14} aria-hidden="true" />
        Unpack
      </Button>
      {asked === null && error !== null && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      {asked !== null && (
        <Modal label={`Unpack ${name}`}>
          <div className={styles.confirm}>
            <h2>Unpack {name}?</h2>
            <p>
              Becomes {asked.parts.length} artifact
              {asked.parts.length === 1 ? "" : "s"}:{" "}
              {asked.parts.map((p) => p.name).join(", ")}.
            </p>
            <p>
              {asked.profiles.length === 0
                ? "No profile holds it."
                : `Profiles that change: ${asked.profiles
                    .map((p) => `${p.name} v${p.from} → v${p.to}`)
                    .join(", ")}.`}
            </p>
            {asked.placementsDropped > 0 && (
              <p>
                It leaves the map of those profiles; the map shows out of date
                until you draw it again.
              </p>
            )}
            <p>Keeps its tags and collections.</p>
            <p>
              Its skills lose the <code>plugin:</code> prefix; a project keeps
              its version until you switch it.
            </p>
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
                onClick={() => void unpack()}
                disabled={busy}
              >
                Unpack
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </>
  );
}
