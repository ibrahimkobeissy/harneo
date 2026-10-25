import { CircleAlert, Eye, TriangleAlert } from "lucide-react";
import { useState } from "react";
import type { Plan } from "../../bindings/Plan";
import type { RefitAction } from "../../bindings/RefitAction";
import type { Row } from "../../bindings/Row";
import { Callout, CodeView, FileTree, markOf } from "../../ui";
import { PluginNotice } from "../projects/PluginNotice";
import styles from "./Refit.module.css";
import { baseName, criticalLosses, lossText, lossUnit } from "./summary";

/** Step 4: the dry run. Only the files that change are listed; a replaced one shows its diff. */
export function PreviewStep({
  plan,
  rows,
  choices,
  displayPath,
  error,
}: {
  plan: Plan;
  rows: readonly Row[];
  choices: Record<string, RefitAction>;
  displayPath: string;
  error: string | null;
}) {
  const losses = criticalLosses(rows, choices);
  const lossFor = (path: string) =>
    losses.find((l) => l.row.item === path)?.loss ?? null;
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
      {losses.length > 0 && (
        <Callout tone="bad" icon={TriangleAlert}>
          <p>
            <b>
              {losses
                .map((l) => `${baseName(l.row.item)}: ${lossText(l.loss)}`)
                .join(" · ")}
            </b>
          </p>
          <p>
            Applying replaces these files, never merges. The originals are
            backed up first. Go back and choose <b>Keep</b> to leave one
            untouched.
          </p>
          <ul className={styles.lostList}>
            {losses.map((l) => (
              <li key={l.row.item}>
                <span className={styles.mono}>{l.row.item}</span>
                <details>
                  <summary>Show lost lines</summary>
                  <div className={styles.lost}>
                    {l.loss.lines.map((line, i) => (
                      <div key={i}>− {line}</div>
                    ))}
                    {l.loss.count > l.loss.lines.length && (
                      <div className={styles.muted}>
                        … {l.loss.count - l.loss.lines.length} more
                      </div>
                    )}
                  </div>
                </details>
              </li>
            ))}
          </ul>
        </Callout>
      )}
      <PluginNotice plan={plan} />
      <div className={styles.split}>
        <FileTree
          label="Files to change"
          files={changes.map((w) => {
            const loss = w.action === "replace" ? lossFor(w.path) : null;
            return loss
              ? {
                  path: w.path,
                  mark: "delete" as const,
                  markText: `replace · ${loss.count} ${loss.unit === "permissionRules" ? (loss.count === 1 ? "rule" : "rules") : lossUnit(loss)} lost`,
                }
              : { path: w.path, mark: markOf(w.action) };
          })}
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
