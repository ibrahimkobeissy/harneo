import { useEffect, useMemo, useState } from "react";
import type { InstalledFile } from "../../bindings/InstalledFile";
import type { ProjectFile } from "../../bindings/ProjectFile";
import { projectFileContent } from "../../lib/ipc";
import { CodeView, FileTree } from "../../ui";
import { messageOf, toBase64 } from "./format";
import styles from "./Projects.module.css";

/** The files Harneo wrote, as a tree with the selected file's current content beside it. */
export function FilesTab({
  projectId,
  files,
}: {
  projectId: bigint;
  files: InstalledFile[];
}) {
  const first =
    files.find((f) => f.path === "HARNESS.md")?.path ?? files[0]?.path ?? null;
  const [picked, setPicked] = useState<string | null>(null);
  const selected = files.find((f) => f.path === picked)?.path ?? first;
  const [loaded, setLoaded] = useState<{
    path: string;
    file: ProjectFile | null;
    error: string | null;
  } | null>(null);

  useEffect(() => {
    if (selected === null) return;
    let live = true;
    projectFileContent(projectId, selected).then(
      (file) => {
        if (live) setLoaded({ path: selected, file, error: null });
      },
      (err: unknown) => {
        if (live)
          setLoaded({ path: selected, file: null, error: messageOf(err) });
      },
    );
    return () => {
      live = false;
    };
  }, [projectId, selected]);

  const tree = useMemo(() => files.map((f) => ({ path: f.path })), [files]);
  if (files.length === 0) {
    return (
      <p className={styles.muted}>Harneo has not written any files here.</p>
    );
  }
  const current = loaded?.path === selected ? loaded : null;
  return (
    <div className={styles.split}>
      <FileTree
        label="Installed files"
        files={tree}
        selected={selected}
        onSelect={setPicked}
      />
      {selected !== null && (
        <div className={styles.viewer}>
          {current?.error && (
            <p className={styles.viewErr} role="alert">
              Could not read {selected}. {current.error}
            </p>
          )}
          {current?.file && (
            <>
              <CodeView
                path={selected}
                bytes={toBase64(current.file.content)}
              />
              {current.file.truncated && (
                <p className={styles.viewNote}>
                  This file is long; only the start is shown.
                </p>
              )}
            </>
          )}
          {!current && (
            <p className={styles.viewNote} role="status">
              Loading {selected}…
            </p>
          )}
        </div>
      )}
    </div>
  );
}
