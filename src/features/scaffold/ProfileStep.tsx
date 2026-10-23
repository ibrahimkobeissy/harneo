import { Layers } from "lucide-react";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { Button, EmptyState, kinds } from "../../ui";
import styles from "./Scaffold.module.css";

/** Step 2: which profile to install, one card each with its file count. */
export function ProfileStep({
  profiles,
  chosen,
  onChoose,
  onCreate,
}: {
  profiles: ProfileSummary[];
  chosen: bigint | null;
  onChoose: (id: bigint) => void;
  onCreate: () => void;
}) {
  if (profiles.length === 0) {
    return (
      <div className={styles.emptyProfiles}>
        <EmptyState icon={Layers} title="No profiles yet">
          A profile is the set of artifacts a project gets. Create one, then
          scaffold from it.
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
            {p.name} <span className={styles.muted}>v{p.version}</span>
          </b>
          {p.description && <span className={styles.sec}>{p.description}</span>}
          <span className={styles.count}>
            {p.counts.map(({ kind, count }) => {
              const meta = kinds[kind];
              const Icon = meta.icon;
              return (
                <span key={kind} className={styles.kindCount}>
                  <Icon size={12} aria-hidden="true" />
                  {count}{" "}
                  {(count === 1 ? meta.label : meta.plural).toLowerCase()}
                </span>
              );
            })}
            <span>
              {p.fileCount} {p.fileCount === 1 ? "file" : "files"}
            </span>
          </span>
        </button>
      ))}
    </div>
  );
}
