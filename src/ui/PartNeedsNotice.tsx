import { Link2 } from "lucide-react";
import type { ArtifactKind } from "../bindings/ArtifactKind";
import { Button } from "./Button";
import { Callout } from "./Callout";

/** A kept plugin part that names a part of its plugin that is not kept (ADR-008). */
export interface PartNeedRow {
  /** Unique among the rows. */
  id: string;
  /** The kept part's name and kind. */
  part: string;
  kind: ArtifactKind;
  /** The dropped part it names. */
  needs: string;
  /** False when the needed part is not in the library, so it cannot be kept from here. */
  canKeep: boolean;
  onKeep: () => void;
  onDropBoth: () => void;
}

/** A command is called with a slash. */
function called(name: string, kind: ArtifactKind): string {
  return kind === "command" ? `/${name}` : name;
}

/**
 * Names each kept plugin part that needs a dropped one, with one click to keep the needed part or
 * drop both. Nothing is decided on its own. Nothing when there is none.
 */
export function PartNeedsNotice({ rows }: { rows: PartNeedRow[] }) {
  if (rows.length === 0) return null;
  return (
    <Callout tone="warn" icon={Link2}>
      <p>
        <b>
          {rows.length === 1
            ? "A kept part needs a part that is not kept."
            : `${rows.length} kept parts need a part that is not kept.`}
        </b>
      </p>
      <ul aria-label="Parts that need a dropped part">
        {rows.map((row) => (
          <li key={row.id}>
            <code>{called(row.part, row.kind)}</code> needs{" "}
            <code>{row.needs}</code>
            {!row.canKeep && " (it is not in your library)"}{" "}
            {row.canKeep && (
              <Button small onClick={row.onKeep}>
                Keep {row.needs}
              </Button>
            )}{" "}
            <Button small onClick={row.onDropBoth}>
              Drop both
            </Button>
          </li>
        ))}
      </ul>
    </Callout>
  );
}
