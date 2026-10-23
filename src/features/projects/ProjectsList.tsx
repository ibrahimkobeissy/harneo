import { FolderGit2, FolderPlus, Layers, Clock, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";
import type { Project } from "../../bindings/Project";
import { profilesList, projectsList } from "../../lib/ipc";
import { Button, Callout, EmptyState, Screen } from "../../ui";
import { ago, messageOf, plural, statusOf } from "./format";
import styles from "./Projects.module.css";
import { StatusPill } from "./StatusPill";
import { Toast, type Notice } from "./Toast";

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

/** The projects Harneo has scaffolded or refitted, with the profile each one runs. */
export function ProjectsScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const versions = useProfileVersions();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(() => {
    const text = (location.state as { toast?: string } | null)?.toast;
    return text ? { text, tone: "ok" } : null;
  });
  const dismiss = useCallback(() => setNotice(null), []);

  useEffect(() => {
    let live = true;
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
  }, []);

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
        <ul className={styles.cards} aria-label="Projects">
          {projects.map((p) => (
            <li key={String(p.id)}>
              <Link to={`/projects/${p.id}`} className={styles.card}>
                <span className={styles.top}>
                  <span className={styles.ico}>
                    <FolderGit2 size={16} aria-hidden="true" />
                  </span>
                  <span className={styles.who}>
                    <span className={styles.name}>{p.name}</span>
                    <span className={styles.path} title={p.path}>
                      {p.path}
                    </span>
                  </span>
                </span>
                <span className={styles.mid}>
                  <Layers size={14} aria-hidden="true" />
                  <span>
                    {p.profileName}{" "}
                    <span className={styles.mono}>v{p.profileVersion}</span>
                  </span>
                  <span className={styles.grow} />
                  <StatusPill status={statusOf(p, versions.get(p.profileId))} />
                </span>
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
      )}
      {notice && <Toast notice={notice} onDismiss={dismiss} />}
    </Screen>
  );
}
