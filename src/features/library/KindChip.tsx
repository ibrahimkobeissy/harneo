import type { ArtifactKind } from "../../bindings/ArtifactKind";
import { kinds } from "./kinds";
import styles from "./Library.module.css";

/** The kind's tinted icon square; the label is read next to it, so the icon is decorative. */
export function KindChip({
  kind,
  small = false,
}: {
  kind: ArtifactKind;
  small?: boolean;
}) {
  const { icon: Icon, tint } = kinds[kind];
  return (
    <span
      className={small ? `${styles.chip} ${styles.chipSm}` : styles.chip}
      data-tint={tint}
    >
      <Icon size={small ? 13 : 16} aria-hidden="true" />
    </span>
  );
}
