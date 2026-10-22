import {
  CircleAlert,
  FolderOpen,
  FolderSearch,
  GitBranch,
  House,
  Info,
  type LucideIcon,
} from "lucide-react";
import { Button } from "../../ui";
import type { ImportFailure } from "./errors";
import styles from "./Import.module.css";

export type SourceChoice = "claude" | "git" | "folder";

export interface SourceForm {
  choice: SourceChoice;
  url: string;
  path: string;
}

const choices: {
  id: SourceChoice;
  icon: LucideIcon;
  title: string;
  text: string;
}[] = [
  {
    id: "claude",
    icon: House,
    title: "Your ~/.claude",
    text: "Skills, commands, agents and settings you already use.",
  },
  {
    id: "git",
    icon: GitBranch,
    title: "A git URL",
    text: "A public pack like Addy's agent-skills. Harneo records URL, branch, subfolder and commit.",
  },
  {
    id: "folder",
    icon: FolderOpen,
    title: "A folder",
    text: "Any repo or folder with a .claude/ directory or loose artifacts.",
  },
];

/** A failed scan is red; "nothing found" is not a failure, so it is a neutral note. */
function FieldError({
  id,
  text,
  neutral = false,
}: {
  id: string;
  text: string;
  neutral?: boolean;
}) {
  if (neutral) {
    return (
      <span className={styles.nothing} id={id} role="status">
        <Info size={15} aria-hidden="true" />
        {text}
      </span>
    );
  }
  return (
    <span className={styles.err} id={id} role="alert">
      <CircleAlert size={13} aria-hidden="true" />
      {text}
    </span>
  );
}

/** Step 1: pick where to import from. A failed scan comes back as `failure` under its field. */
export function SourceStep({
  form,
  failure,
  onChange,
  onBrowse,
}: {
  form: SourceForm;
  failure: ImportFailure | null;
  onChange: (next: Partial<SourceForm>) => void;
  onBrowse: () => void;
}) {
  const urlFailure = failure?.field === "url" ? failure : null;
  const pathFailure = failure?.field === "path" ? failure : null;
  const urlError = urlFailure?.message ?? null;
  const pathError = pathFailure?.message ?? null;
  return (
    <>
      <div className={styles.choices}>
        {choices.map(({ id, icon: Icon, title, text }) => (
          <button
            key={id}
            type="button"
            className={styles.choice}
            aria-pressed={form.choice === id}
            onClick={() => onChange({ choice: id })}
          >
            <span className={styles.ic}>
              <Icon size={16} aria-hidden="true" />
            </span>
            <b>{title}</b>
            <span className={styles.sec}>{text}</span>
          </button>
        ))}
      </div>
      {form.choice === "claude" && failure && (
        <div className={styles.field}>
          <FieldError
            id="claude-error"
            text={failure.message}
            neutral={failure.neutral}
          />
        </div>
      )}
      {form.choice === "git" && (
        <div className={styles.field}>
          <label htmlFor="import-url">Repository URL</label>
          <input
            id="import-url"
            className={styles.input}
            placeholder="github.com/owner/repo"
            value={form.url}
            autoComplete="off"
            spellCheck={false}
            aria-invalid={urlError && !urlFailure?.neutral ? true : undefined}
            aria-describedby="import-url-note"
            onChange={(e) => onChange({ url: e.target.value })}
          />
          {urlError ? (
            <FieldError
              id="import-url-note"
              text={urlError}
              neutral={urlFailure?.neutral}
            />
          ) : (
            <span className={styles.hint} id="import-url-note">
              Optionally add a branch and subfolder:{" "}
              <span className={styles.mono}>
                github.com/owner/repo/tree/main/skills
              </span>
            </span>
          )}
        </div>
      )}
      {form.choice === "folder" && (
        <div className={styles.field}>
          <label htmlFor="import-path">Folder</label>
          <div className={styles.inputRow}>
            <input
              id="import-path"
              className={styles.input}
              placeholder="~/code/my-team-harness"
              value={form.path}
              autoComplete="off"
              spellCheck={false}
              aria-invalid={
                pathError && !pathFailure?.neutral ? true : undefined
              }
              aria-describedby="import-path-note"
              onChange={(e) => onChange({ path: e.target.value })}
            />
            <Button onClick={onBrowse}>
              <FolderSearch size={14} aria-hidden="true" />
              Browse…
            </Button>
          </div>
          {pathError ? (
            <FieldError
              id="import-path-note"
              text={pathError}
              neutral={pathFailure?.neutral}
            />
          ) : (
            <span className={styles.hint} id="import-path-note">
              Harneo looks for .claude/, CLAUDE.md, .mcp.json and loose SKILL.md
              files.
            </span>
          )}
        </div>
      )}
    </>
  );
}
