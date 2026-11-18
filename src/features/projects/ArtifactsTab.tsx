import type { ProjectArtifact } from "../../bindings/ProjectArtifact";
import { KindChip, kinds } from "../../ui";
import styles from "./Projects.module.css";

/** What the project's profile installs and why, read from Harneo's database, not from a file. */
export function ArtifactsTab({ artifacts }: { artifacts: ProjectArtifact[] }) {
  if (artifacts.length === 0) {
    return <p className={styles.muted}>This profile holds no artifacts.</p>;
  }
  return (
    <ul className={styles.artifacts} aria-label="Installed artifacts">
      {artifacts.map((a) => (
        <li key={a.id}>
          <KindChip kind={a.kind} small />
          <span className={styles.artifactName}>{a.name}</span>
          <span className={styles.muted}>{kinds[a.kind].label}</span>
          <span className={styles.artifactWhy}>{a.why}</span>
        </li>
      ))}
    </ul>
  );
}
