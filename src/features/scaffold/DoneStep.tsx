import { CircleCheck, FolderGit2, FolderPlus } from "lucide-react";
import { Button, PluginsInstall } from "../../ui";
import styles from "./Scaffold.module.css";

/** The success summary after the files are written. */
export function DoneStep({
  folder,
  profile,
  written,
  plugins,
  projectId,
  onOpen,
  onAgain,
}: {
  folder: string;
  profile: string;
  written: number;
  /** The profile holds plugins, which are enabled in settings but never copied. */
  plugins: boolean;
  /** The registered project, whose plugins are installed here. */
  projectId: bigint;
  onOpen: () => void;
  onAgain: () => void;
}) {
  return (
    <div className={styles.result}>
      <div className={styles.okIcon}>
        <CircleCheck size={22} aria-hidden="true" />
      </div>
      <h2>
        Scaffolded {folder} with {profile}
      </h2>
      <p>
        {written} {written === 1 ? "file" : "files"} written, including{" "}
        <span className={styles.mono}>HARNESS.md</span>. The project is in your
        registry.
      </p>
      {plugins && <PluginsInstall projectId={projectId} />}
      <div className={styles.actions}>
        <Button onClick={onOpen}>
          <FolderGit2 size={14} aria-hidden="true" />
          Open project
        </Button>
        <Button variant="primary" onClick={onAgain}>
          <FolderPlus size={14} aria-hidden="true" />
          Scaffold another
        </Button>
      </div>
    </div>
  );
}
