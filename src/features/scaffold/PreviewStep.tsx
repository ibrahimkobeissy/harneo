import { CircleAlert, Eye } from "lucide-react";
import { useState } from "react";
import type { Plan } from "../../bindings/Plan";
import { Callout, CodeView, FileTree, markOf } from "../../ui";
import { PluginNotice } from "../projects/PluginNotice";
import styles from "./Scaffold.module.css";

/** Step 3: the dry run. Every file is new; click one to read what will be written. */
export function PreviewStep({
  plan,
  displayPath,
  error,
}: {
  plan: Plan;
  displayPath: string;
  error: string | null;
}) {
  const first =
    plan.writes.find((w) => w.path === "HARNESS.md") ?? plan.writes[0];
  const [picked, setPicked] = useState<string | null>(null);
  const current = plan.writes.find((w) => w.path === picked) ?? first;
  const count = plan.writes.length;
  return (
    <>
      {error ? (
        <Callout tone="bad" icon={CircleAlert}>
          <p>
            <b>Nothing was written.</b> {error}
          </p>
          <p>
            Pick a folder you can write to, or fix its permissions and try
            again.
          </p>
        </Callout>
      ) : (
        <Callout icon={Eye}>
          <p>
            <b>Dry run.</b> These {count} {count === 1 ? "file" : "files"} will
            be written to <span className={styles.mono}>{displayPath}</span>. No
            harness files exist there yet, so nothing is replaced. Click any
            file to read its full content.
          </p>
        </Callout>
      )}
      <PluginNotice plan={plan} />
      <div className={styles.split}>
        <FileTree
          label="Files to write"
          files={plan.writes.map((w) => ({
            path: w.path,
            mark: markOf(w.action),
          }))}
          selected={current?.path ?? null}
          onSelect={setPicked}
        />
        {current && <CodeView path={current.path} bytes={current.bytes} />}
      </div>
    </>
  );
}
