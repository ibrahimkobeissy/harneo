import { ArrowLeft, CircleAlert, Download } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import type { ApplyReport } from "../../bindings/ApplyReport";
import type { PlanTicket } from "../../bindings/PlanTicket";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import {
  homeClaudeDir,
  installApply,
  pickFolder,
  profilesList,
  scaffoldCheck,
  scaffoldPlan,
} from "../../lib/ipc";
import {
  Button,
  Callout,
  Screen,
  Stepper,
  WizardActions,
  hasPlugins,
} from "../../ui";
import { DoneStep } from "./DoneStep";
import { FolderStep } from "./FolderStep";
import { PreviewStep } from "./PreviewStep";
import { ProfileStep } from "./ProfileStep";
import styles from "./Scaffold.module.css";

const steps = ["Folder", "Profile", "Preview", "Done"] as const;

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Errors about the folder itself belong on the Folder step; the rest stay where they came from. */
function aboutFolder(message: string): boolean {
  return /has a harness|not a folder|not writable/.test(message);
}

/** `~` is expanded here since the core reads paths as given. */
async function expand(path: string): Promise<string> {
  const trimmed = path.trim();
  if (trimmed !== "~" && !/^~[\\/]/.test(trimmed)) return trimmed;
  const home = (await homeClaudeDir()).replace(/[\\/]\.claude$/, "");
  return home + trimmed.slice(1);
}

function baseName(path: string): string {
  const parts = path.trim().split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Scaffold: pick a folder and a profile, see every file, then write them. */
export function ScaffoldScreen() {
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [path, setPath] = useState("");
  const [folderError, setFolderError] = useState<{
    message: string;
    hasHarness: boolean;
  } | null>(null);
  const [checking, setChecking] = useState(false);
  const [profiles, setProfiles] = useState<ProfileSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [profileId, setProfileId] = useState<bigint | null>(null);
  const [planning, setPlanning] = useState(false);
  const [planError, setPlanError] = useState<string | null>(null);
  const [plan, setPlan] = useState<PlanTicket | null>(null);
  const [applying, setApplying] = useState(false);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [report, setReport] = useState<ApplyReport | null>(null);

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
    return () => {
      live = false;
    };
  }, []);

  const profile = profiles?.find((p) => p.id === profileId) ?? null;

  async function browse() {
    const chosen = await pickFolder().catch(() => null);
    if (chosen) {
      setPath(chosen);
      setFolderError(null);
    }
  }

  /** Step 1 Continue: a folder that cannot be scaffolded is refused here, before a profile is picked. */
  async function checkFolder() {
    setChecking(true);
    try {
      await scaffoldCheck(await expand(path));
      setFolderError(null);
      setStep(1);
    } catch (error) {
      const message = messageOf(error);
      setFolderError({ message, hasHarness: /has a harness/.test(message) });
    } finally {
      setChecking(false);
    }
  }

  async function preview() {
    if (profileId === null) return;
    setPlanning(true);
    setPlanError(null);
    try {
      setPlan(await scaffoldPlan(await expand(path), profileId));
      setApplyError(null);
      setStep(2);
    } catch (error) {
      const message = messageOf(error);
      if (aboutFolder(message)) {
        setFolderError({ message, hasHarness: /has a harness/.test(message) });
        setStep(0);
      } else {
        setPlanError(message);
      }
    } finally {
      setPlanning(false);
    }
  }

  async function write() {
    if (!plan) return;
    setApplying(true);
    setApplyError(null);
    try {
      setReport(await installApply(plan));
      setStep(3);
    } catch (error) {
      setApplyError(messageOf(error));
    } finally {
      setApplying(false);
    }
  }

  function again() {
    setStep(0);
    setPath("");
    setFolderError(null);
    setProfileId(null);
    setPlan(null);
    setReport(null);
    setApplyError(null);
    setPlanError(null);
  }

  const crumb = (
    <div className={styles.crumb}>
      <Link to="/projects">
        <ArrowLeft size={14} aria-hidden="true" />
        Projects
      </Link>
    </div>
  );
  const fileCount = plan?.writes.length ?? 0;
  const label = profile ? `${profile.name} v${profile.version}` : "";

  const foot =
    step < 3 ? (
      <WizardActions
        back={
          step > 0 ? (
            <Button
              disabled={applying || planning}
              onClick={() => {
                setApplyError(null);
                setStep(step - 1);
              }}
            >
              Back
            </Button>
          ) : undefined
        }
        note={
          step === 2 ? (
            <>
              {path.trim()} · {label}
            </>
          ) : undefined
        }
      >
        {step === 0 && (
          <Button
            variant="primary"
            disabled={path.trim() === "" || checking}
            onClick={() => void checkFolder()}
          >
            {checking ? "Checking…" : "Continue"}
          </Button>
        )}
        {step === 1 && (
          <Button
            variant="primary"
            disabled={profileId === null || planning}
            onClick={() => void preview()}
          >
            {planning ? "Preparing…" : "Continue"}
          </Button>
        )}
        {step === 2 && (
          <Button
            variant="primary"
            disabled={applying || fileCount === 0}
            onClick={() => void write()}
          >
            <Download size={14} aria-hidden="true" />
            {applying
              ? "Writing…"
              : `Write ${fileCount} ${fileCount === 1 ? "file" : "files"}`}
          </Button>
        )}
      </WizardActions>
    ) : undefined;

  return (
    <Screen
      title="Scaffold project"
      narrow
      crumb={crumb}
      toolbar={<Stepper steps={steps} at={step} />}
      foot={foot}
    >
      {step === 0 && (
        <FolderStep
          path={path}
          error={folderError}
          onChange={(next) => {
            setPath(next);
            setFolderError(null);
          }}
          onBrowse={() => void browse()}
          onRefit={() =>
            void navigate(`/refit?path=${encodeURIComponent(path.trim())}`)
          }
        />
      )}
      {step === 1 && (
        <>
          {(planError ?? loadError) && (
            <Callout tone="bad" icon={CircleAlert}>
              <p>{planError ?? loadError}</p>
            </Callout>
          )}
          {profiles ? (
            <ProfileStep
              profiles={profiles}
              chosen={profileId}
              onChoose={(id) => {
                setProfileId(id);
                setPlanError(null);
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
      {step === 2 && plan && (
        <PreviewStep plan={plan} displayPath={path.trim()} error={applyError} />
      )}
      {step === 3 && report && plan && (
        <DoneStep
          folder={baseName(path)}
          profile={label}
          written={report.written}
          plugins={hasPlugins(plan)}
          onOpen={() => void navigate(`/projects/${report.projectId}`)}
          onAgain={again}
        />
      )}
    </Screen>
  );
}
