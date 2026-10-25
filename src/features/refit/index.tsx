import { ArrowLeft, Check, CircleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";
import type { ApplyReport } from "../../bindings/ApplyReport";
import type { Comparison } from "../../bindings/Comparison";
import type { PlanTicket } from "../../bindings/PlanTicket";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import type { Project } from "../../bindings/Project";
import type { RefitAction } from "../../bindings/RefitAction";
import {
  backupRestore,
  homeClaudeDir,
  installApply,
  pickFolder,
  profilesList,
  projectsList,
  refitCompare,
  refitPlan,
} from "../../lib/ipc";
import {
  announce,
  Button,
  Callout,
  Screen,
  Stepper,
  WizardActions,
  hasPlugins,
} from "../../ui";
import { CompareStep } from "./CompareStep";
import { DoneStep } from "./DoneStep";
import { FolderStep } from "./FolderStep";
import { PreviewStep } from "./PreviewStep";
import { ProfileStep } from "./ProfileStep";
import styles from "./Refit.module.css";
import { baseName, criticalLosses, samePath, totalLoss } from "./summary";

const steps = ["Folder", "Profile", "Compare", "Preview", "Done"] as const;

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Errors about the folder itself belong on the Folder step. */
function aboutFolder(message: string): boolean {
  return /not a folder|no Claude harness/.test(message);
}

/** A raw core error gets a next step: what to try, since the core's text is for logs. */
function withAdvice(message: string): string {
  return /permission denied|read-only|access is denied/i.test(message)
    ? `${message} Check that you can write to the project folder, or pick another folder.`
    : `${message} Nothing was changed. Go back and try again, or pick another folder or profile.`;
}

/** `~` is expanded here since the core reads paths as given. */
async function expand(path: string): Promise<string> {
  const trimmed = path.trim();
  if (trimmed !== "~" && !/^~[\\/]/.test(trimmed)) return trimmed;
  const home = (await homeClaudeDir()).replace(/[\\/]\.claude$/, "");
  return home + trimmed.slice(1);
}

/** A comparison with nothing of the project's own: the folder has no harness to refit. */
function hasNoHarness(comparison: Comparison): boolean {
  return comparison.rows.every((r) => r.inProject === "absent");
}

/** Refit: compare an existing harness with a profile item by item, choose, preview, apply. */
export function RefitScreen() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [step, setStep] = useState(0);
  const [path, setPath] = useState(params.get("path") ?? "");
  const [folderError, setFolderError] = useState<string | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [profiles, setProfiles] = useState<ProfileSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [profileId, setProfileId] = useState<bigint | null>(null);
  const [busy, setBusy] = useState(false);
  const [stepError, setStepError] = useState<string | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const [choices, setChoices] = useState<Record<string, RefitAction>>({});
  const [plan, setPlan] = useState<PlanTicket | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [report, setReport] = useState<ApplyReport | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [restored, setRestored] = useState(false);
  const [restoredCount, setRestoredCount] = useState(0);
  const [restoreError, setRestoreError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    profilesList().then(
      (list) => {
        if (live) setProfiles(list);
      },
      (error: unknown) => {
        if (live) setLoadError(messageOf(error));
      },
    );
    projectsList().then(
      (list) => {
        if (live) setProjects(list ?? []);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const registered = projects.find((p) => samePath(p.path, path)) ?? null;
  // A registered project's current profile is preselected until the user picks another.
  const currentId =
    registered && profiles?.some((p) => p.id === registered.profileId)
      ? registered.profileId
      : null;
  const chosenId = profileId ?? currentId;
  const profile = profiles?.find((p) => p.id === chosenId) ?? null;

  function changePath(next: string) {
    setPath(next);
    setFolderError(null);
    setComparison(null);
    setChoices({});
  }

  async function browse() {
    const chosen = await pickFolder().catch(() => null);
    if (chosen) changePath(chosen);
  }

  function toProfile() {
    setStepError(null);
    setStep(1);
  }

  async function compare() {
    if (chosenId === null) return;
    setBusy(true);
    setStepError(null);
    try {
      const result = await refitCompare(await expand(path), chosenId);
      if (hasNoHarness(result)) {
        setFolderError(
          `${path.trim()} has no Claude harness. Scaffold it instead.`,
        );
        setStep(0);
        return;
      }
      setComparison(result);
      setChoices({});
      setStep(2);
    } catch (error) {
      const message = messageOf(error);
      if (aboutFolder(message)) {
        setFolderError(message);
        setStep(0);
      } else {
        setStepError(withAdvice(message));
      }
    } finally {
      setBusy(false);
    }
  }

  async function preview() {
    if (chosenId === null) return;
    setBusy(true);
    setStepError(null);
    try {
      setPlan(await refitPlan(await expand(path), chosenId, choices));
      setApplyError(null);
      setStep(3);
    } catch (error) {
      setStepError(withAdvice(messageOf(error)));
    } finally {
      setBusy(false);
    }
  }

  async function apply() {
    if (!plan) return;
    setBusy(true);
    setApplyError(null);
    try {
      setReport(await installApply(plan));
      setStep(4);
    } catch (error) {
      setApplyError(messageOf(error));
    } finally {
      setBusy(false);
    }
  }

  async function restore() {
    if (!report || report.backupId === null) return;
    setRestoring(true);
    setRestoreError(null);
    try {
      const back = await backupRestore(report.backupId);
      setRestoredCount(back.written);
      setRestored(true);
      setConfirming(false);
      announce("Originals restored");
    } catch (error) {
      setRestoreError(messageOf(error));
    } finally {
      setRestoring(false);
    }
  }

  const crumb = (
    <div className={styles.crumb}>
      <Link to="/projects">
        <ArrowLeft size={14} aria-hidden="true" />
        Projects
      </Link>
    </div>
  );
  const nothing = step === 2 && comparison?.nothingToChange === true;
  const changing =
    plan?.writes.filter((w) => w.action !== "keep" && w.action !== "skip")
      .length ?? 0;
  const losses = criticalLosses(comparison?.rows ?? [], choices);
  const applyLabel =
    losses.length > 0
      ? `Apply refit (loses ${totalLoss(losses)})`
      : "Apply refit";
  const label = profile ? `${profile.name} v${profile.version}` : "";
  const flowLabel = comparison
    ? `${comparison.profileName} v${comparison.profileVersion}`
    : label;

  const foot =
    step < 4 ? (
      <WizardActions
        back={
          step > 0 ? (
            <Button
              disabled={busy}
              onClick={() => {
                setApplyError(null);
                setStepError(null);
                setStep(step - 1);
              }}
            >
              Back
            </Button>
          ) : undefined
        }
        note={
          step === 3 ? (
            <>
              {path.trim()} · {flowLabel}
            </>
          ) : undefined
        }
      >
        {step === 0 && (
          <Button
            variant="primary"
            disabled={path.trim() === ""}
            onClick={toProfile}
          >
            Continue
          </Button>
        )}
        {step === 1 && (
          <Button
            variant="primary"
            disabled={chosenId === null || busy}
            onClick={() => void compare()}
          >
            {busy ? "Comparing…" : "Continue"}
          </Button>
        )}
        {step === 2 && !nothing && (
          <Button
            variant="primary"
            disabled={busy}
            onClick={() => void preview()}
          >
            {busy ? "Preparing…" : "Preview changes"}
          </Button>
        )}
        {step === 3 && (
          <Button
            variant="primary"
            disabled={busy || changing === 0}
            onClick={() => void apply()}
          >
            <Check size={14} aria-hidden="true" />
            {busy ? "Applying…" : applyLabel}
          </Button>
        )}
      </WizardActions>
    ) : undefined;

  return (
    <Screen
      title="Refit project"
      crumb={crumb}
      toolbar={<Stepper steps={steps} at={step} />}
      foot={foot}
    >
      {step === 0 && (
        <FolderStep
          path={path}
          error={folderError}
          projects={projects}
          onChange={changePath}
          onBrowse={() => void browse()}
          onScaffold={() => void navigate("/scaffold")}
        />
      )}
      {step === 1 && (
        <>
          {(stepError ?? loadError) && (
            <Callout tone="bad" icon={CircleAlert}>
              <p>{stepError ?? loadError}</p>
            </Callout>
          )}
          {profiles ? (
            <ProfileStep
              profiles={profiles}
              chosen={chosenId}
              current={registered?.profileId ?? null}
              onChoose={(id) => {
                setProfileId(id);
                setStepError(null);
                setComparison(null);
                setChoices({});
              }}
              onCreate={() => void navigate("/profiles/new")}
            />
          ) : (
            !loadError && (
              <div className={styles.status} role="status">
                <span className={styles.spinner} aria-hidden="true" />
                Loading profiles…
              </div>
            )
          )}
        </>
      )}
      {step === 2 && comparison && (
        <>
          {stepError && (
            <Callout tone="bad" icon={CircleAlert}>
              <p>{stepError}</p>
            </Callout>
          )}
          <CompareStep
            comparison={comparison}
            choices={choices}
            onChoose={(item, action) =>
              setChoices((prev) => ({ ...prev, [item]: action }))
            }
          />
        </>
      )}
      {step === 3 && plan && (
        <PreviewStep
          plan={plan}
          rows={comparison?.rows ?? []}
          choices={choices}
          displayPath={path.trim()}
          error={applyError}
        />
      )}
      {step === 4 && report && (
        <DoneStep
          folder={baseName(path)}
          profile={flowLabel}
          backedUp={report.backedUp}
          plugins={plan ? hasPlugins(plan) : false}
          backupFolder={report.backupFolder}
          projectId={String(report.projectId)}
          restoredCount={restoredCount}
          addedCount={
            plan?.writes.filter((w) => w.action === "new").length ?? 0
          }
          canRestore={report.backupId !== null}
          confirming={confirming}
          restoring={restoring}
          restored={restored}
          error={restoreError}
          onAskRestore={() => setConfirming(true)}
          onCancel={() => setConfirming(false)}
          onRestore={() => void restore()}
          onProjects={() => void navigate("/projects")}
        />
      )}
    </Screen>
  );
}
