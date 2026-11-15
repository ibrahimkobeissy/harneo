import type { ArtifactKind } from "../../bindings/ArtifactKind";
import { KindChip, kinds } from "../../ui";
import styles from "./Import.module.css";

/** The artifact's type, icon and label, as the first column of a review row. */
export function KindCell({ kind }: { kind: ArtifactKind }) {
  return (
    <span className={styles.kindCell} data-testid="kind-cell">
      <KindChip kind={kind} small />
      <span>{kinds[kind].label}</span>
    </span>
  );
}
