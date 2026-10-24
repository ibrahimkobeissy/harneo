import { Layers } from "lucide-react";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { Button, EmptyState } from "../../ui";
import styles from "./Refit.module.css";
import clamp from "../../ui/Clamp.module.css";

/** Step 2: the profile to compare with; the project's current one comes preselected. */
export function ProfileStep({
  profiles,
  chosen,
  current,
  onChoose,
  onCreate,
}: {
  profiles: ProfileSummary[];
  chosen: bigint | null;
  current: bigint | null;
  onChoose: (id: bigint) => void;
  onCreate: () => void;
}) {
  if (profiles.length === 0) {
    return (
      <div className={styles.emptyProfiles}>
        <EmptyState icon={Layers} title="No profiles yet">
          A profile is the set of artifacts a project gets. Create one, then
          refit from it.
        </EmptyState>
        <Button variant="primary" onClick={onCreate}>
          Create a profile
        </Button>
      </div>
    );
  }
  return (
    <div className={styles.choices}>
      {profiles.map((p) => (
        <button
          key={String(p.id)}
          type="button"
          className={styles.choice}
          aria-pressed={chosen === p.id}
          onClick={() => onChoose(p.id)}
        >
          <span className={styles.ic}>
            <Layers size={16} aria-hidden="true" />
          </span>
          <b>
            <span className={clamp.one} title={p.name}>
              {p.name}
            </span>{" "}
            <span className={styles.muted}>v{p.version}</span>
          </b>
          {p.description && (
            <span
              className={`${clamp.two} ${styles.sec}`}
              title={p.description}
            >
              {p.description}
            </span>
          )}
          <span className={styles.count}>
            {p.fileCount} {p.fileCount === 1 ? "file" : "files"}
            {current === p.id ? " · current profile" : ""}
          </span>
        </button>
      ))}
    </div>
  );
}
