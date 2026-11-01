import { useEffect, useState } from "react";
import type { ApplyReport } from "../../bindings/ApplyReport";
import type { PlanTicket } from "../../bindings/PlanTicket";
import type { Project } from "../../bindings/Project";
import { installApply, projectForgetMissing, removalPlan } from "../../lib/ipc";
import { DialogShell } from "./DialogShell";
import { messageOf, plural } from "./format";
import { PlanFiles } from "./PlanFiles";
import styles from "./Projects.module.css";

/** The clean-removal confirm: how many files go, how many originals come back. */
export function RemoveDialog({
  project,
  backupDir,
  onClose,
  onRemoved,
}: {
  project: Project;
  backupDir: string | null;
  onClose: () => void;
  onRemoved: (report: ApplyReport, restored: number) => void;
}) {
  const [plan, setPlan] = useState<PlanTicket | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    removalPlan(project.id).then(
      (next) => {
        if (live) setPlan(next);
      },
      (err: unknown) => {
        if (live) setPlanError(messageOf(err));
      },
    );
    return () => {
      live = false;
    };
  }, [project.id]);

  const restored = plan?.restored.length ?? 0;
  const removed = plan
    ? plan.writes.filter((w) => w.action === "delete").length
    : 0;
  const edited = plan?.editedInProject.length ?? 0;

  async function apply() {
    if (!plan) return;
    setApplying(true);
    setApplyError(null);
    try {
      onRemoved(await installApply(plan), restored);
    } catch (err) {
      setApplyError(messageOf(err));
      setApplying(false);
    }
  }

  return (
    <DialogShell
      title={`Remove the harness from ${project.name}?`}
      submit={applying ? "Removing…" : "Remove and restore originals"}
      variant="danger-solid"
      busy={applying}
      canSubmit={plan !== null}
      onSubmit={() => void apply()}
      onClose={onClose}
    >
      {plan && (
        <p className={styles.text}>
          Removes the {plural(removed, "file")} Harneo wrote and restores the{" "}
          {plural(restored, "original")} it backed up.{" "}
          <span className={styles.mono}>CLAUDE.local.md</span> and{" "}
          <span className={styles.mono}>settings.local.json</span> are never
          touched. The project leaves the registry.
        </p>
      )}
      {plan && (
        <PlanFiles
          plan={plan}
          show={["deleted", "restored", "edited"]}
          backupDir={backupDir}
        />
      )}
      {plan && edited > 0 && (
        <p className={styles.note}>
          {edited === 1 ? "One file was" : `${edited} files were`} edited in the
          project since Harneo wrote {edited === 1 ? "it" : "them"}. Edits are
          backed up before they go.
        </p>
      )}
      {!plan && !planError && (
        <p className={styles.muted} role="status">
          Counting the files…
        </p>
      )}
      {planError && (
        <p className={styles.err} role="alert">
          Could not prepare the removal. {planError}
        </p>
      )}
      {applyError && (
        <p className={styles.err} role="alert">
          Nothing was changed. {applyError}
        </p>
      )}
    </DialogShell>
  );
}

/**
 * The removal for a project whose folder is gone: there is nothing to delete or restore, so
 * Harneo only stops tracking it. Its backups stay.
 */
export function ForgetDialog({
  project,
  onClose,
  onForgotten,
}: {
  project: Project;
  onClose: () => void;
  onForgotten: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function forget() {
    setBusy(true);
    setError(null);
    try {
      await projectForgetMissing(project.id);
      onForgotten();
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <DialogShell
      title={`Remove ${project.name} from Harneo?`}
      submit={busy ? "Removing…" : "Remove from Harneo"}
      variant="danger-solid"
      busy={busy}
      onSubmit={() => void forget()}
      onClose={onClose}
    >
      <p className={styles.text}>
        The folder <span className={styles.mono}>{project.path}</span> is gone,
        so there is nothing to delete or restore. Harneo stops tracking the
        project; nothing on disk is touched, and its backups stay listed under
        removed projects.
      </p>
      {error && (
        <p className={styles.err} role="alert">
          Nothing was changed. {error}
        </p>
      )}
    </DialogShell>
  );
}
