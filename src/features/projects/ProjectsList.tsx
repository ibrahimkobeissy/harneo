import { FolderGit2, FolderPlus, Layers, Clock, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";
import type { Project } from "../../bindings/Project";
import { useChanged } from "../../lib/useChanged";
import { useDriftSummaries } from "../../lib/useDriftSummaries";
import {
  openBackupsFolder,
  profilesList,
  projectsList,
  projectsRemoved,
} from "../../lib/ipc";
import { Button, Callout, EmptyState, Screen } from "../../ui";
import { ago, messageOf, plural, statusOf, type Status } from "./format";
import styles from "./Projects.module.css";
import { StatusPill } from "./StatusPill";
import { Toast, type Notice } from "./Toast";
import clamp from "../../ui/Clamp.module.css";

/** Every profile's current version by id; a failed load just means no "available" pills. */
export function useProfileVersions(): ReadonlyMap<bigint, number> {
  const [versions, setVersions] = useState<ReadonlyMap<bigint, number>>(
    new Map(),
  );
  useEffect(() => {
    let live = true;
    profilesList().then(
      (list) => {
        if (live) setVersions(new Map(list.map((p) => [p.id, p.version])));
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);
  return versions;
}

/** Attention first: Missing, Drift, a newer profile, newer library copies, then in sync. */
const RANK: Record<Status["kind"], number> = {
  missing: 0,
  drift: 1,
  available: 2,
  library: 3,
  sync: 4,
};

/** The projects Harneo has scaffolded or refitted, with the profile each one runs. */
export function ProjectsScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const versions = useProfileVersions();
  const drift = useDriftSummaries();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [removed, setRemoved] = useState<Project[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(() => {
    const state = location.state as {
      toast?: string;
      folder?: string;
    } | null;
    return state?.toast
      ? { text: state.toast, tone: "ok", folder: state.folder }
      : null;
  });
  const dismiss = useCallback(() => setNotice(null), []);
  const [attempt, setAttempt] = useState(0);
  useChanged(useCallback(() => setAttempt((n) => n + 1), []));

  useEffect(() => {
    let live = true;
    projectsRemoved().then(
      (list) => {
        if (live) setRemoved(Array.isArray(list) ? list : []);
      },
      () => undefined,
    );
    projectsList().then(
      (list) => {
        if (live) setProjects(list);
      },
      (err: unknown) => {
        if (live) setError(messageOf(err));
      },
    );
    return () => {
      live = false;
    };
  }, [attempt]);

  const ordered = useMemo(
    () =>
      (projects ?? [])
        .map((project) => ({
          project,
          status: statusOf(
            project,
            versions.get(project.profileId),
            drift.get(project.id),
          ),
        }))
        .sort((a, b) => RANK[a.status.kind] - RANK[b.status.kind]),
    [projects, versions, drift],
  );
  const attention = ordered.filter((o) => o.status.kind !== "sync").length;

  const actions = (
    <>
      <Button onClick={() => void navigate("/refit")}>
        <RefreshCw size={14} aria-hidden="true" />
        Refit existing
      </Button>
      <Button variant="primary" onClick={() => void navigate("/scaffold")}>
        <FolderPlus size={14} aria-hidden="true" />
        Scaffold new
      </Button>
    </>
  );

  return (
    <Screen title="Projects" actions={actions}>
      {error && (
        <Callout tone="bad" icon={FolderGit2}>
          <p>Could not load the projects. {error}</p>
        </Callout>
      )}
      {projects && projects.length === 0 && (
        <EmptyState icon={FolderGit2} title="No projects yet">
          Scaffold a new project from a profile, or refit one that already has a
          harness.
        </EmptyState>
      )}
      {projects && projects.length > 0 && (
        <>
          {attention > 0 && (
            <p className={styles.attention}>{attention} need attention</p>
          )}
          <ul className={styles.cards} aria-label="Projects">
            {ordered.map(({ project: p, status }) => (
              <li key={String(p.id)}>
                <Link to={`/projects/${p.id}`} className={styles.card}>
                  <span className={styles.top}>
                    <span className={styles.ico}>
                      <FolderGit2 size={16} aria-hidden="true" />
                    </span>
                    <span className={styles.who}>
                      <span
                        className={`${clamp.one} ${styles.name}`}
                        title={p.name}
                      >
                        {p.name}
                      </span>
                      <span className={styles.path} title={p.path}>
                        {p.path}
                      </span>
                    </span>
                  </span>
                  <span className={styles.mid}>
                    <Layers size={14} aria-hidden="true" />
                    <span
                      className={`${clamp.one} ${styles.profile}`}
                      title={p.profileName}
                    >
                      {p.profileName}{" "}
                      <span className={styles.mono}>v{p.profileVersion}</span>
                    </span>
                    <span className={styles.grow} />
                    <StatusPill status={status} />
                  </span>
                  {status.kind === "missing" && (
                    <span className={styles.missingNote}>
                      The folder is gone, so nothing can be compared. Put it
                      back, or open the project to remove it.
                    </span>
                  )}
                  <span className={styles.foot}>
                    <Clock size={14} aria-hidden="true" />
                    <span>Applied {ago(p.appliedAt)}</span>
                    <span className={styles.grow} />
                    <span className={styles.muted}>
                      {plural(p.fileCount, "file")}
                    </span>
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        </>
      )}
      {removed.length > 0 && (
        <section className={styles.removed} aria-labelledby="removed-heading">
          <h2 id="removed-heading">Removed projects</h2>
          <p className={styles.muted}>
            Their backups are kept, so you can still restore a file.
          </p>
          <ul aria-label="Removed projects">
            {removed.map((p) => (
              <li key={String(p.id)}>
                <Link to={`/projects/${p.id}`}>{p.name}</Link>{" "}
                <span className={styles.path} title={p.path}>
                  {p.path}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {notice && (
        <Toast
          notice={notice}
          onDismiss={dismiss}
          onOpenFolder={(f) =>
            void openBackupsFolder(f).catch((err: unknown) =>
              setNotice({
                text: `Could not open the folder. ${messageOf(err)}`,
                tone: "bad",
              }),
            )
          }
        />
      )}
    </Screen>
  );
}
