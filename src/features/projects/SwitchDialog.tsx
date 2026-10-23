import { useEffect, useState } from "react";
import type { ApplyReport } from "../../bindings/ApplyReport";
import type { PlanTicket } from "../../bindings/PlanTicket";
import type { Project } from "../../bindings/Project";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { installApply, switchPlan } from "../../lib/ipc";
import { DialogShell } from "./DialogShell";
import { messageOf } from "./format";
import { groupPlan, PlanFiles } from "./PlanFiles";
import styles from "./Projects.module.css";

/** The Switch / Upgrade modal: the difference as Add, Remove and Update, then "Apply difference". */
export function SwitchDialog({
  project,
  profiles,
  upgrade,
  backupDir,
  onClose,
  onApplied,
}: {
  project: Project;
  profiles: ProfileSummary[];
  /** Same profile, newer version, so no profile list. */
  upgrade: boolean;
  backupDir: string | null;
  onClose: () => void;
  onApplied: (
    report: ApplyReport,
    to: { name: string; version: number },
  ) => void;
}) {
  const others = profiles.filter((p) => p.id !== project.profileId);
  const [chosen, setChosen] = useState<bigint | null>(
    upgrade ? project.profileId : (others[0]?.id ?? null),
  );
  const [result, setResult] = useState<{
    for: bigint;
    plan: PlanTicket | null;
    error: string | null;
  } | null>(null);
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);

  useEffect(() => {
    if (chosen === null) return;
    let live = true;
    switchPlan(project.id, chosen).then(
      (next) => {
        if (live) setResult({ for: chosen, plan: next, error: null });
      },
      (err: unknown) => {
        if (live) setResult({ for: chosen, plan: null, error: messageOf(err) });
      },
    );
    return () => {
      live = false;
    };
  }, [project.id, chosen]);

  // A result for a profile that is no longer picked counts as not loaded yet.
  const fresh = result?.for === chosen ? result : null;
  const plan = fresh?.plan ?? null;
  const planError = fresh?.error ?? null;
  const target = profiles.find((p) => p.id === chosen);
  const edited = plan?.editedInProject.length ?? 0;
  const changing = plan
    ? Object.values(groupPlan(plan)).reduce((n, list) => n + list.length, 0)
    : 0;

  async function apply() {
    if (!plan) return;
    setApplying(true);
    setApplyError(null);
    try {
      const report = await installApply(plan);
      onApplied(report, {
        name: plan.profileName,
        version: plan.profileVersion,
      });
    } catch (err) {
      setApplyError(messageOf(err));
      setApplying(false);
    }
  }

  const title = upgrade
    ? `Upgrade ${project.name}`
    : `Switch ${project.name} to another profile`;
  const from = `${project.profileName} v${project.profileVersion}`;
  const to = plan
    ? `${plan.profileName} v${plan.profileVersion}`
    : target
      ? `${target.name} v${target.version}`
      : "";

  return (
    <DialogShell
      title={title}
      sub="Only the difference is applied. Every replaced file is backed up first."
      submit={applying ? "Applying…" : "Apply difference"}
      busy={applying}
      canSubmit={plan !== null}
      wide
      onSubmit={() => void apply()}
      onClose={onClose}
    >
      {!upgrade &&
        (others.length === 0 ? (
          <p className={styles.muted}>
            There is no other profile to switch to.
          </p>
        ) : (
          <fieldset className={styles.optlist}>
            <legend className={styles.srOnly}>Switch to</legend>
            {others.map((p) => (
              <label
                key={String(p.id)}
                className={styles.opt}
                data-on={chosen === p.id ? "true" : undefined}
              >
                <input
                  type="radio"
                  name="switch-profile"
                  checked={chosen === p.id}
                  onChange={() => setChosen(p.id)}
                />
                <span>
                  <b>
                    {p.name} <span className={styles.mono}>v{p.version}</span>
                  </b>
                  {p.description && (
                    <span className={styles.sec}>{p.description}</span>
                  )}
                </span>
              </label>
            ))}
          </fieldset>
        ))}
      {planError && (
        <p className={styles.err} role="alert">
          Could not work out the difference. {planError}
        </p>
      )}
      {chosen !== null && !plan && !planError && (
        <p className={styles.muted} role="status">
          Working out the difference…
        </p>
      )}
      {plan && (
        <>
          <p className={styles.fromTo}>
            {from} → {to}
          </p>
          <PlanFiles
            plan={plan}
            show={["add", "update", "restored", "deleted", "edited"]}
            backupDir={backupDir}
          />
          {changing === 0 && (
            <p className={styles.rowPlain}>
              Same files; only the version changes.
            </p>
          )}
        </>
      )}
      {plan && edited > 0 && (
        <p className={styles.note}>
          {edited === 1 ? "One file was" : `${edited} files were`} edited in the
          project since Harneo wrote them. They are backed up before they
          change.
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
