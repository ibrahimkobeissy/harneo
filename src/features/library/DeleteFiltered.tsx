import { useEffect, useState } from "react";
import type { Filter } from "../../bindings/Filter";
import type { RemovalPlan } from "../../bindings/RemovalPlan";
import { Modal } from "../../app/Modal";
import { libraryRemovalPlan, libraryRemoveMany } from "../../lib/ipc";
import { Button } from "../../ui";
import { kinds } from "./kinds";
import styles from "./DeleteFiltered.module.css";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; plan: RemovalPlan };

/**
 * Confirms deleting every artifact the filter matches (all pages, not only the visible one).
 * The artifacts listed are exactly the ones removed, in one all-or-nothing call.
 */
export function DeleteFilteredDialog({
  filter,
  onClose,
  onDeleted,
}: {
  filter: Filter;
  onClose: () => void;
  onDeleted: () => void;
}) {
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The filter is fixed for the life of the dialog: the plan is fetched once.
  const [fixed] = useState(filter);

  useEffect(() => {
    let current = true;
    libraryRemovalPlan(fixed).then(
      (plan) => {
        if (current) setLoad({ status: "ready", plan });
      },
      (err: unknown) => {
        if (current) setLoad({ status: "error", message: String(err) });
      },
    );
    return () => {
      current = false;
    };
  }, [fixed]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented || busy) return;
      onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  const artifacts = load.status === "ready" ? load.plan.artifacts : [];
  const count = artifacts.length;
  const used = artifacts.filter((a) => a.profiles.length > 0);
  const profileNames = [
    ...new Set(used.flatMap((a) => a.profiles.map((p) => p.name))),
  ];
  const noun = count === 1 ? "artifact" : "artifacts";

  async function confirm() {
    setBusy(true);
    setError(null);
    try {
      await libraryRemoveMany(artifacts.map((a) => a.id));
      onDeleted();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  return (
    <Modal
      label={
        load.status === "ready" ? `Delete ${count} ${noun}` : "Delete artifacts"
      }
    >
      <div className={styles.confirm}>
        <h2>
          {load.status === "ready"
            ? `Delete ${count} ${noun}`
            : "Delete artifacts"}
        </h2>
        {load.status === "loading" && <p>Finding the artifacts…</p>}
        {load.status === "error" && (
          <p role="alert" className={styles.error}>
            {load.message}
          </p>
        )}
        {load.status === "ready" && (
          <>
            <p>
              This deletes {count === 1 ? "this artifact" : "these artifacts"}{" "}
              from your library, with their tags and collection entries. The
              originals at their sources are not touched.
            </p>
            <ul className={styles.list} aria-label="Artifacts to delete">
              {artifacts.map((a) => (
                <li key={a.id} className={styles.item}>
                  <span className={styles.name} title={a.name}>
                    {a.name}
                  </span>
                  <span className={styles.kind}>{kinds[a.kind].label}</span>
                </li>
              ))}
            </ul>
            {used.length > 0 && (
              <p role="note" className={styles.warn}>
                {used.length} of them {used.length === 1 ? "is" : "are"} used by{" "}
                {profileNames.length === 1 ? "a profile" : "profiles"} (
                {profileNames.join(", ")}). Those profiles get a new version
                without them.
              </p>
            )}
          </>
        )}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
        <div className={styles.actions}>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant="danger-solid"
            onClick={() => void confirm()}
            disabled={busy || load.status !== "ready" || count === 0}
          >
            Delete
          </Button>
        </div>
      </div>
    </Modal>
  );
}
