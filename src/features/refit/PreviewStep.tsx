import { CircleAlert, Eye } from "lucide-react";
import { useState } from "react";
import type { Plan } from "../../bindings/Plan";
import { Callout, CodeView, FileTree, markOf } from "../../ui";
import styles from "./Refit.module.css";

/** Step 4: the dry run. Only the files that change are listed; a replaced one shows its diff. */
export function PreviewStep({
  plan,
  displayPath,
  error,
}: {
  plan: Plan;
  displayPath: string;
  error: string | null;
}) {
  const changes = plan.writes.filter(
    (w) => w.action !== "keep" && w.action !== "skip",
  );
  const [picked, setPicked] = useState<string | null>(null);
  const current = changes.find((w) => w.path === picked) ?? changes[0];
  const count = changes.length;
  return (
    <>
      {error ? (
        <Callout tone="bad" icon={CircleAlert}>
          <p>
            <b>Nothing was changed.</b> {error}
          </p>
        </Callout>
      ) : (
        <Callout icon={Eye}>
          <p>
            <b>Dry run.</b> {count} {count === 1 ? "file" : "files"} will change
            in <span className={styles.mono}>{displayPath}</span>.{" "}
            {plan.backupCount} backed up first. Select a file to see its content
            diff.
          </p>
        </Callout>
      )}
      <div className={styles.split}>
        <FileTree
          label="Files to change"
          files={changes.map((w) => ({ path: w.path, mark: markOf(w.action) }))}
          selected={current?.path ?? null}
          onSelect={setPicked}
        />
        {current &&
          (current.action === "delete" ? (
            <div className={styles.removed}>
              <b className={styles.mono}>{current.path}</b>
              <p>
                This file is removed from the project. Its original is backed up
                first.
              </p>
            </div>
          ) : (
            <CodeView
              path={current.path}
              bytes={current.bytes}
              hunks={current.diff}
            />
          ))}
      </div>
    </>
  );
}
