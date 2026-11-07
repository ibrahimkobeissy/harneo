import type { ArtifactFit } from "../../bindings/ArtifactFit";
import { Badge, verdictLabel, verdictTone } from "../../ui";
import styles from "./Library.module.css";

/** How many profiles' verdicts a card shows before "+n". */
const SHOWN = 2;

/** One badge per profile with a saved fit of the artifact; a stale fit says "out of date". */
export function FitBadges({ fits }: { fits: ArtifactFit[] }) {
  if (fits.length === 0) return null;
  return (
    <span className={styles.tileFits} aria-label="Harnfit verdicts">
      {fits.slice(0, SHOWN).map((f) => (
        <Badge key={String(f.profileId)} tone={verdictTone(f.verdict)}>
          {f.profileName}: {verdictLabel(f.verdict)}
          {f.stale && " · out of date"}
        </Badge>
      ))}
      {fits.length > SHOWN && <Badge>+{fits.length - SHOWN}</Badge>}
    </span>
  );
}
