import { useEffect, useState } from "react";
import type { Hunk } from "../../bindings/Hunk";
import type { Plan } from "../../bindings/Plan";
import type { WriteAction } from "../../bindings/WriteAction";
import { driftDiff, driftReapplyPlan, installApply } from "../../lib/ipc";
import { Badge, type BadgeTone, CodeView } from "../../ui";
import { DialogShell } from "./DialogShell";
import { messageOf, plural } from "./format";
import styles from "./Projects.module.css";

const labels: Partial<Record<WriteAction, { text: string; tone: BadgeTone }>> =
  {
    replace: { text: "Replace", tone: "warn" },
    new: { text: "Restore", tone: "ok" },
    delete: { text: "Remove", tone: "bad" },
  };

/** Confirms putting the profile's version of some files back: the list, then "Re-apply". */
export function ReapplyDialog({
  projectId,
  projectName,
  paths,
  title,
  onClose,
  onDone,
}: {
  projectId: bigint;
  projectName: string;
  /** The files to put back; all drifted files when `null`. */
  paths: string[] | null;
  title: string;
  onClose: () => void;
  onDone: (count: number) => void;
}) {
  const [plan, setPlan] = useState<Plan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [diff, setDiff] = useState<{ path: string; hunks: Hunk[] } | null>(
    null,
  );
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    driftReapplyPlan(projectId, paths ?? undefined).then(
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
  }, [projectId, paths]);

  const writes = (plan?.writes ?? []).filter((w) => labels[w.action]);
  // One file gets its diff, so the user sees what the replacement overwrites.
  const only = paths?.length === 1 ? paths[0] : undefined;
  const shown = writes.some((w) => w.path === only && w.action === "replace")
    ? only
    : undefined;
  useEffect(() => {
    if (shown === undefined) return;
    let live = true;
    driftDiff(projectId, shown).then(
      (hunks) => {
        if (live) setDiff({ path: shown, hunks });
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [projectId, shown]);
  const hunks = diff && diff.path === shown ? diff.hunks : null;

  async function apply() {
    if (!plan) return;
    setApplying(true);
    setApplyError(null);
    try {
      await installApply(plan);
      onDone(writes.length);
    } catch (err) {
      setApplyError(messageOf(err));
      setApplying(false);
    }
  }

  return (
    <DialogShell
      title={title}
      sub={`${projectName}'s versions are backed up first, then the profile's versions are written.`}
      submit={
        applying
          ? "Re-applying…"
          : only !== undefined
            ? "Replace file"
            : "Re-apply"
      }
      busy={applying}
      canSubmit={plan !== null && writes.length > 0}
      onSubmit={() => void apply()}
      onClose={onClose}
    >
      {planError && (
        <p className={styles.err} role="alert">
          Could not work out what to re-apply. {planError}
        </p>
      )}
      {!plan && !planError && (
        <p className={styles.muted} role="status">
          Working out what to re-apply…
        </p>
      )}
      {plan && (
        <section className={styles.diff} aria-label="Files to re-apply">
          <header className={styles.diffHead}>
            {plural(writes.length, "file")}
          </header>
          <ul className={styles.diffGroup}>
            {writes.map((w) => {
              const label = labels[w.action];
              return (
                <li key={w.path} className={styles.diffRow}>
                  <span className={styles.diffTag}>
                    {label && <Badge tone={label.tone}>{label.text}</Badge>}
                  </span>
                  <span className={styles.diffPath}>{w.path}</span>
                </li>
              );
            })}
          </ul>
          {writes.length === 0 && (
            <p className={styles.rowPlain}>Nothing to re-apply.</p>
          )}
        </section>
      )}
      {shown !== undefined && hunks && (
        <div className={styles.reapplyDiff}>
          <CodeView path={shown} bytes={null} hunks={hunks} />
        </div>
      )}
      {applyError && (
        <p className={styles.err} role="alert">
          Nothing was changed. {applyError}
        </p>
      )}
    </DialogShell>
  );
}
