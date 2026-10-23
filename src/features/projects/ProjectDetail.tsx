import {
  ArrowLeft,
  ArrowLeftRight,
  CircleArrowUp,
  Download,
  FolderGit2,
  GitCompare,
  FolderTree,
  Archive,
  SquarePen,
  Undo2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import type { Backup } from "../../bindings/Backup";
import type { DriftReport } from "../../bindings/DriftReport";
import type { InstalledFile } from "../../bindings/InstalledFile";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import type { Project } from "../../bindings/Project";
import { useAppInfo } from "../../app/useAppInfo";
import {
  driftCheck,
  openFolder,
  profilesList,
  projectBackups,
  projectFiles,
  projectGet,
} from "../../lib/ipc";
import { useChanged } from "../../lib/useChanged";
import { Button, Callout, EmptyState, Panel, Screen } from "../../ui";
import { BackupsTab } from "./BackupsTab";
import { DriftTab } from "./DriftTab";
import { FilesTab } from "./FilesTab";
import { ago, messageOf, statusOf } from "./format";
import styles from "./Projects.module.css";
import { RemoveDialog } from "./RemoveDialog";
import { StatusPill } from "./StatusPill";
import { SwitchDialog } from "./SwitchDialog";
import { Toast, type Notice } from "./Toast";

type Tab = "drift" | "files" | "backups";
type Dialog = "switch" | "upgrade" | "remove" | null;

function parseId(raw: string | undefined): bigint | null {
  return raw !== undefined && /^\d+$/.test(raw) ? BigInt(raw) : null;
}

/** One project: the profile it runs, the files Harneo wrote, and its backups. */
export function ProjectScreen() {
  const navigate = useNavigate();
  const id = parseId(useParams().id);
  const app = useAppInfo();
  const [project, setProject] = useState<Project | null>(null);
  const [files, setFiles] = useState<InstalledFile[]>([]);
  const [backups, setBackups] = useState<Backup[]>([]);
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [report, setReport] = useState<DriftReport | null>(null);
  const [picked, setTab] = useState<Tab | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const dismiss = useCallback(() => setNotice(null), []);
  const [attempt, setAttempt] = useState(0);
  const reload = useCallback(() => setAttempt((n) => n + 1), []);

  useChanged((scope) => {
    if (scope.scope === "library" || (id !== null && scope.id === Number(id)))
      reload();
  });

  useEffect(() => {
    if (id === null) return;
    let live = true;
    Promise.all([
      projectGet(id),
      projectFiles(id),
      projectBackups(id),
      profilesList().catch(() => [] as ProfileSummary[]),
      driftCheck(id).catch(() => null),
    ]).then(
      ([p, f, b, list, drift]) => {
        if (!live) return;
        setProject(p);
        setFiles(f);
        setBackups(b);
        setProfiles(list);
        setReport(drift ?? null);
        setLoadError(null);
      },
      (err: unknown) => {
        if (live) setLoadError(messageOf(err));
      },
    );
    return () => {
      live = false;
    };
  }, [id, attempt]);

  const say = useCallback(
    (text: string, tone: "ok" | "bad") => setNotice({ text, tone }),
    [],
  );

  if (id === null || loadError) {
    return (
      <Screen title="Project">
        <EmptyState icon={FolderGit2} title="Project not found">
          {id === null ? "There is no project with that address." : loadError}{" "}
          <Link to="/projects">Back to projects</Link>
        </EmptyState>
      </Screen>
    );
  }
  if (!project) {
    return (
      <Screen title="Project">
        <p className={styles.muted} role="status">
          Loading the project…
        </p>
      </Screen>
    );
  }

  const current = profiles.find((p) => p.id === project.profileId);
  const status = statusOf(
    project,
    current?.version,
    report && {
      status: report.status,
      drifted: report.files.length,
      libraryNewer: report.libraryNewer.length,
    },
  );
  const tab: Tab =
    picked ??
    (report && (report.files.length > 0 || report.status === "missing")
      ? "drift"
      : "files");
  const dataDir = app.status === "ready" ? app.info.dataDir : null;

  async function open() {
    if (!project) return;
    try {
      await openFolder(project.path);
    } catch (err) {
      say(`Could not open the folder. ${messageOf(err)}`, "bad");
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
  const actions = (
    <div className={styles.actions}>
      <Button onClick={() => void open()}>
        <SquarePen size={14} aria-hidden="true" />
        Open
      </Button>
      <Button
        onClick={() =>
          void navigate(`/import?path=${encodeURIComponent(project.path)}`)
        }
      >
        <Download size={14} aria-hidden="true" />
        Import from this project
      </Button>
      <Button onClick={() => setDialog("switch")}>
        <ArrowLeftRight size={14} aria-hidden="true" />
        Switch profile
      </Button>
      <Button variant="danger" onClick={() => setDialog("remove")}>
        <Undo2 size={14} aria-hidden="true" />
        Remove profile
      </Button>
    </div>
  );

  return (
    <Screen
      title={project.name}
      crumb={crumb}
      actions={actions}
      toolbar={
        <div className={styles.pathLine} title={project.path}>
          {project.path}
        </div>
      }
    >
      {status.kind === "available" && (
        <Callout icon={CircleArrowUp}>
          <div className={styles.upgrade}>
            <div>
              <b>
                {project.profileName} is at v{status.version}; this project runs
                v{project.profileVersion}.
              </b>
              <p>
                Upgrading applies only the difference, with a preview first.
              </p>
            </div>
            <Button
              small
              variant="primary"
              onClick={() => setDialog("upgrade")}
            >
              Upgrade to v{status.version}
            </Button>
          </div>
        </Callout>
      )}
      <div className={styles.stats}>
        <Panel title="Profile">
          <div className={styles.stat}>
            <Link to={`/profiles/${project.profileId}`}>
              {project.profileName}
            </Link>{" "}
            <span className={styles.mono}>v{project.profileVersion}</span>
          </div>
        </Panel>
        <Panel title="Status">
          <div className={styles.stat}>
            <StatusPill status={status} />
          </div>
        </Panel>
        <Panel title="Last applied">
          <div className={styles.stat}>{ago(project.appliedAt)}</div>
        </Panel>
        <Panel title="Backups">
          <div className={styles.stat}>{backups.length}</div>
        </Panel>
      </div>
      <div className={styles.tabs} role="tablist" aria-label="Project">
        <button
          type="button"
          role="tab"
          id="tab-drift"
          aria-selected={tab === "drift"}
          aria-controls="panel-drift"
          onClick={() => setTab("drift")}
        >
          <GitCompare size={14} aria-hidden="true" />
          Drift{" "}
          <span className={styles.muted}>{report?.files.length ?? 0}</span>
        </button>
        <button
          type="button"
          role="tab"
          id="tab-files"
          aria-selected={tab === "files"}
          aria-controls="panel-files"
          onClick={() => setTab("files")}
        >
          <FolderTree size={14} aria-hidden="true" />
          Installed files <span className={styles.muted}>{files.length}</span>
        </button>
        <button
          type="button"
          role="tab"
          id="tab-backups"
          aria-selected={tab === "backups"}
          aria-controls="panel-backups"
          onClick={() => setTab("backups")}
        >
          <Archive size={14} aria-hidden="true" />
          Backups <span className={styles.muted}>{backups.length}</span>
        </button>
      </div>
      <div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {tab === "drift" ? (
          report ? (
            <DriftTab
              project={project}
              report={report}
              onChanged={reload}
              onNotice={say}
            />
          ) : (
            <p className={styles.muted}>Could not check this project.</p>
          )
        ) : tab === "files" ? (
          <FilesTab projectId={project.id} files={files} />
        ) : (
          <BackupsTab
            backups={backups}
            dataDir={dataDir}
            onChanged={reload}
            onNotice={say}
          />
        )}
      </div>
      {(dialog === "switch" || dialog === "upgrade") && (
        <SwitchDialog
          project={project}
          profiles={profiles}
          upgrade={dialog === "upgrade"}
          onClose={() => setDialog(null)}
          onApplied={(_, to) => {
            setDialog(null);
            say(
              `${project.name} now runs ${to.name} v${to.version}. Replaced files are backed up.`,
              "ok",
            );
            reload();
          }}
        />
      )}
      {dialog === "remove" && (
        <RemoveDialog
          project={project}
          onClose={() => setDialog(null)}
          onRemoved={(report, restored) => {
            const where = report.backupFolder
              ? ` Backed up in ${report.backupFolder}.`
              : "";
            void navigate("/projects", {
              state: {
                toast: `Removed the harness from ${project.name}; ${restored} ${restored === 1 ? "original" : "originals"} restored.${where}`,
              },
            });
          }}
        />
      )}
      {notice && <Toast notice={notice} onDismiss={dismiss} />}
    </Screen>
  );
}
