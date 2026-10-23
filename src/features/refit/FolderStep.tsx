import { CircleAlert, FolderGit2, FolderSearch, Plus } from "lucide-react";
import type { Project } from "../../bindings/Project";
import { Button } from "../../ui";
import styles from "./Refit.module.css";

/** Step 1: the project to refit, typed, browsed for, or picked from the registered ones. */
export function FolderStep({
  path,
  error,
  projects,
  onChange,
  onBrowse,
  onScaffold,
}: {
  path: string;
  error: string | null;
  projects: Project[];
  onChange: (path: string) => void;
  onBrowse: () => void;
  onScaffold: () => void;
}) {
  return (
    <>
      <div className={styles.field}>
        <label htmlFor="refit-path">Project folder</label>
        <div className={styles.inputRow}>
          <input
            id="refit-path"
            className={styles.input}
            value={path}
            placeholder="~/work/site"
            aria-invalid={error ? true : undefined}
            aria-describedby="refit-path-note"
            onChange={(e) => onChange(e.target.value)}
          />
          <Button onClick={onBrowse}>
            <FolderSearch size={14} aria-hidden="true" />
            Browse…
          </Button>
        </div>
        {error ? (
          <>
            <span className={styles.err} id="refit-path-note" role="alert">
              <CircleAlert size={13} aria-hidden="true" />
              {error}
            </span>
            <Button small className={styles.inline} onClick={onScaffold}>
              <Plus size={13} aria-hidden="true" />
              Scaffold it instead
            </Button>
          </>
        ) : (
          <span className={styles.hint} id="refit-path-note">
            A project that already has a Claude harness. Harneo compares it with
            a profile item by item.
          </span>
        )}
      </div>
      {projects.length > 0 && (
        <section className={styles.group} aria-labelledby="refit-known">
          <header>
            <FolderSearch size={14} aria-hidden="true" />
            <span id="refit-known">Folders with a harness</span>
          </header>
          {projects.map((p) => (
            <button
              key={String(p.id)}
              type="button"
              className={styles.known}
              aria-pressed={p.path === path.trim()}
              onClick={() => onChange(p.path)}
            >
              <FolderGit2 size={14} aria-hidden="true" />
              <span className={styles.mono}>{p.path}</span>
              <span className={styles.grow} />
              <span className={styles.muted}>
                registered · {p.profileName} v{p.profileVersion}
              </span>
            </button>
          ))}
        </section>
      )}
    </>
  );
}
