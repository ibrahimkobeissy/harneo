import {
  ArrowUpFromLine,
  CircleCheck,
  Download,
  FileDiff,
  FileX,
  RotateCcw,
  TriangleAlert,
  Trash2,
  CircleArrowUp,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router";
import type { DriftFile } from "../../bindings/DriftFile";
import type { DriftReport } from "../../bindings/DriftReport";
import type { Hunk } from "../../bindings/Hunk";
import type { Project } from "../../bindings/Project";
import { driftDiff, driftPromote, driftPromoteAdded } from "../../lib/ipc";
import {
  Button,
  Callout,
  CodeView,
  EmptyState,
  FileTree,
  type TreeFile,
} from "../../ui";
import { DialogShell } from "./DialogShell";
import { messageOf, plural } from "./format";
import styles from "./Projects.module.css";
import { ReapplyDialog } from "./ReapplyDialog";

type Dialog =
  | { kind: "all" }
  | { kind: "one"; paths: string[] }
  | { kind: "library" }
  | { kind: "promote"; path: string }
  | null;

const PROMOTE_WHY_ID = "drift-promote-why";

function markOf(file: DriftFile): Pick<TreeFile, "mark" | "markText"> {
  return file.change.type === "edited"
    ? {
        mark: "replace",
        markText: `+${file.change.plus} −${file.change.minus}`,
      }
    : { mark: "delete", markText: "deleted" };
}

/** What differs between the project and what Harneo wrote, with the two ways out. */
export function DriftTab({
  project,
  report,
  onChanged,
  onNotice,
  onForget,
}: {
  project: Project;
  report: DriftReport;
  onChanged: () => void;
  onNotice: (text: string, tone: "ok" | "bad") => void;
  /** The folder is gone: take the project off Harneo's list. */
  onForget: () => void;
}) {
  const navigate = useNavigate();
  const drifted = useMemo(
    () => report.files.filter((f) => f.change.type !== "added"),
    [report.files],
  );
  const added = useMemo(
    () => report.files.filter((f) => f.change.type === "added"),
    [report.files],
  );
  const tree = useMemo(
    () => drifted.map((f) => ({ path: f.path, ...markOf(f) })),
    [drifted],
  );
  const [picked, setPicked] = useState<string | null>(null);
  const file = drifted.find((f) => f.path === picked) ?? drifted[0] ?? null;
  const [dialog, setDialog] = useState<Dialog>(null);
  const [diff, setDiff] = useState<{
    path: string;
    hunks: Hunk[];
    error: string | null;
  } | null>(null);
  const label = `${project.profileName} v${project.profileVersion}`;

  const selected = file?.path ?? null;
  const deleted = file?.change.type === "deleted";
  useEffect(() => {
    if (selected === null || deleted) return;
    let live = true;
    driftDiff(project.id, selected).then(
      (hunks) => {
        if (live) setDiff({ path: selected, hunks, error: null });
      },
      (err: unknown) => {
        if (live) setDiff({ path: selected, hunks: [], error: messageOf(err) });
      },
    );
    return () => {
      live = false;
    };
  }, [project.id, selected, deleted, report]);

  const done = (text: string) => {
    setDialog(null);
    onNotice(text, "ok");
    onChanged();
  };

  async function importAdded(path: string) {
    try {
      const found = await driftPromoteAdded(project.id, path);
      const query = new URLSearchParams({ path: project.path });
      if (found.candidate) query.set("select", found.candidate);
      void navigate(`/import?${query.toString()}`);
    } catch (err) {
      onNotice(`Could not prepare the import. ${messageOf(err)}`, "bad");
    }
  }

  const current = diff?.path === selected ? diff : null;
  // The core runs promote's own checks for each drifted file, so every refusal is known
  // before the click.
  const promoteWhy = file?.promoteBlocked ?? null;

  return (
    <>
      {report.status === "missing" && (
        <Callout tone="bad" icon={FileX}>
          <div className={styles.upgrade}>
            <div>
              <b>The folder for {project.name} is gone.</b>
              <p>
                Nothing can be compared until it is back at{" "}
                <span className={styles.mono}>{project.path}</span>. If it is
                gone for good, remove the project from Harneo.
              </p>
            </div>
            <Button small onClick={onForget}>
              <Trash2 size={14} aria-hidden="true" />
              Remove from Harneo
            </Button>
          </div>
        </Callout>
      )}
      {drifted.length > 0 && (
        <Callout tone="warn" icon={TriangleAlert}>
          <div className={styles.upgrade}>
            <div>
              <b>
                {plural(drifted.length, "file")} in {project.name} no longer
                match {label}.
              </b>
              <p>
                Re-apply the profile (the project's versions are backed up
                first), or promote a change into the library so every project
                can get it.
              </p>
            </div>
            <Button small onClick={() => setDialog({ kind: "all" })}>
              <RotateCcw size={14} aria-hidden="true" />
              Re-apply all
            </Button>
          </div>
        </Callout>
      )}
      {report.libraryNewer.length > 0 && (
        <Callout icon={CircleArrowUp}>
          <div className={styles.upgrade}>
            <div>
              <b>
                Library has newer copies of{" "}
                {plural(report.libraryNewer.length, "file")}.
              </b>
              <p>Re-apply to bring {project.name} up to date.</p>
            </div>
            <Button small onClick={() => setDialog({ kind: "library" })}>
              <RotateCcw size={14} aria-hidden="true" />
              Re-apply
            </Button>
          </div>
        </Callout>
      )}
      {report.files.length === 0 && report.status !== "missing" && (
        <EmptyState icon={CircleCheck} title="No drift">
          Every file in {project.name} matches {label}.
        </EmptyState>
      )}
      {file && (
        <div className={styles.split}>
          <FileTree
            label="Drifted files"
            files={tree}
            selected={selected}
            onSelect={setPicked}
          />
          <div className={styles.viewer}>
            <div className={styles.vbar}>
              <FileDiff size={14} aria-hidden="true" />
              {(deleted || !current || current.error) && (
                <span className={styles.vpath}>{file.path}</span>
              )}
              <span className={styles.grow} />
              <span title={promoteWhy ?? undefined}>
                <Button
                  small
                  disabled={promoteWhy !== null}
                  aria-describedby={promoteWhy ? PROMOTE_WHY_ID : undefined}
                  onClick={() =>
                    setDialog({ kind: "promote", path: file.path })
                  }
                >
                  <ArrowUpFromLine size={14} aria-hidden="true" />
                  Promote to library
                </Button>
              </span>
              <Button
                small
                onClick={() => setDialog({ kind: "one", paths: [file.path] })}
              >
                <RotateCcw size={14} aria-hidden="true" />
                Replace with the profile's version
              </Button>
            </div>
            {promoteWhy && (
              <p id={PROMOTE_WHY_ID} className={styles.viewNote}>
                {promoteWhy}
              </p>
            )}
            {deleted && !promoteWhy && (
              <p className={styles.viewNote}>
                This file was deleted in the project. Re-apply puts it back.
              </p>
            )}
            {current?.error && (
              <p className={styles.viewErr} role="alert">
                Could not compare {file.path}. {current.error}
              </p>
            )}
            {!deleted && !current && (
              <p className={styles.viewNote} role="status">
                Comparing {file.path}…
              </p>
            )}
            {current && !current.error && (
              <CodeView path={file.path} bytes={null} hunks={current.hunks} />
            )}
          </div>
        </div>
      )}
      {added.length > 0 && (
        <section className={styles.addedBox} aria-label="Added in project">
          <h3 className={styles.addedHead}>
            Added in project{" "}
            <span className={styles.muted}>{added.length}</span>
          </h3>
          <p className={styles.footnote}>
            Harneo did not write these. Import one to make it a library
            artifact.
          </p>
          <ul className={styles.group}>
            {added.map((f) => (
              <li key={f.path} className={styles.row}>
                <span className={styles.diffPath}>{f.path}</span>
                <Button small onClick={() => void importAdded(f.path)}>
                  <Download size={14} aria-hidden="true" />
                  Import to library
                </Button>
              </li>
            ))}
          </ul>
        </section>
      )}
      {dialog?.kind === "all" && (
        <ReapplyDialog
          projectId={project.id}
          projectName={project.name}
          paths={null}
          title={`Re-apply ${label} to ${project.name}?`}
          onClose={() => setDialog(null)}
          onDone={(n) =>
            done(`Re-applied ${plural(n, "file")}; originals backed up.`)
          }
        />
      )}
      {dialog?.kind === "one" && (
        <ReapplyDialog
          projectId={project.id}
          projectName={project.name}
          paths={dialog.paths}
          title={`Replace ${dialog.paths[0]?.split("/").pop()} with the profile's version?`}
          onClose={() => setDialog(null)}
          onDone={() =>
            done(
              `Replaced ${dialog.paths[0]?.split("/").pop()} with the profile's version; the project's version is backed up.`,
            )
          }
        />
      )}
      {dialog?.kind === "library" && (
        <ReapplyDialog
          projectId={project.id}
          projectName={project.name}
          paths={report.libraryNewer}
          title="Bring in the library's newer copies?"
          onClose={() => setDialog(null)}
          onDone={(n) =>
            done(`Re-applied ${plural(n, "file")}; originals backed up.`)
          }
        />
      )}
      {dialog?.kind === "promote" && (
        <PromoteDialog
          projectId={project.id}
          path={dialog.path}
          onClose={() => setDialog(null)}
          onDone={() =>
            done(
              "Promoted to the library. Other projects get it when they re-apply.",
            )
          }
        />
      )}
    </>
  );
}

function PromoteDialog({
  projectId,
  path,
  onClose,
  onDone,
}: {
  projectId: bigint;
  path: string;
  onClose: () => void;
  onDone: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function promote() {
    setBusy(true);
    setError(null);
    try {
      await driftPromote(projectId, path);
      onDone();
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }
  return (
    <DialogShell
      title="Promote this change to the library?"
      submit={busy ? "Promoting…" : "Promote to library"}
      busy={busy}
      onSubmit={() => void promote()}
      onClose={onClose}
    >
      <p className={styles.text}>
        The library copy of{" "}
        <span className={styles.mono}>{path.split("/").pop()}</span> becomes the
        project's version. Profiles that include it can then push it to their
        other projects. The previous library copy is backed up first.
      </p>
      {error && (
        <p className={styles.err} role="alert">
          Nothing was changed. {error}
        </p>
      )}
    </DialogShell>
  );
}
