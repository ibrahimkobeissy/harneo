import { useEffect, useState } from "react";
import type { ApplyReport } from "../../bindings/ApplyReport";
import type { Plan } from "../../bindings/Plan";
import type { Project } from "../../bindings/Project";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import type { Write } from "../../bindings/Write";
import type { WriteAction } from "../../bindings/WriteAction";
import { installApply, switchPlan } from "../../lib/ipc";
import { Badge, type BadgeTone } from "../../ui";
import { DialogShell } from "./DialogShell";
import { messageOf } from "./format";
import styles from "./Projects.module.css";

const groups: {
  title: string;
  action: WriteAction;
  label: string;
  tone: BadgeTone;
}[] = [
  { title: "Add", action: "new", label: "Add", tone: "ok" },
  { title: "Remove", action: "delete", label: "Remove", tone: "bad" },
  { title: "Update", action: "replace", label: "Update", tone: "warn" },
];

/** The Switch / Upgrade modal: the difference as Add, Remove and Update, then "Apply difference". */
export function SwitchDialog({
  project,
  profiles,
  upgrade,
  onClose,
  onApplied,
}: {
  project: Project;
  profiles: ProfileSummary[];
  /** Same profile, newer version, so no profile list. */
  upgrade: boolean;
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
    plan: Plan | null;
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
  const edited = new Set(plan?.editedInProject ?? []);
  const changing =
    plan?.writes.filter((w) => w.action !== "keep" && w.action !== "skip") ??
    [];

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
        <section className={styles.diff} aria-label="Difference">
          <header className={styles.diffHead}>
            {from} → {to}
          </header>
          {groups.map((g) => (
            <Group
              key={g.action}
              {...g}
              writes={plan.writes.filter((w) => w.action === g.action)}
              edited={edited}
            />
          ))}
          {changing.length === 0 && (
            <p className={styles.rowPlain}>
              Same files; only the version changes.
            </p>
          )}
        </section>
      )}
      {plan && edited.size > 0 && (
        <p className={styles.note}>
          {edited.size === 1 ? "One file was" : `${edited.size} files were`}{" "}
          edited in the project since Harneo wrote them. They are backed up
          before they change.
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

function Group({
  title,
  label,
  tone,
  writes,
  edited,
}: {
  title: string;
  label: string;
  tone: BadgeTone;
  writes: Write[];
  edited: ReadonlySet<string>;
}) {
  if (writes.length === 0) return null;
  return (
    <ul className={styles.diffGroup} aria-label={title}>
      {writes.map((w) => (
        <li key={w.path} className={styles.diffRow}>
          <span className={styles.diffTag}>
            <Badge tone={tone}>{label}</Badge>
          </span>
          <span className={styles.diffPath}>{w.path}</span>
          {edited.has(w.path) && <Badge tone="warn">Edited in project</Badge>}
        </li>
      ))}
    </ul>
  );
}
