import { Replace } from "lucide-react";
import type { StayWholeWarning } from "../bindings/StayWholeWarning";
import { useArtifactPanel } from "../lib/artifactPanel";
import { Button } from "./Button";
import { Callout } from "./Callout";

/**
 * Plugins whose parts a profile holds although they only work whole (ADR-006): names each plugin
 * and why, with a way to *Replace with the whole plugin* (the part's panel in the Library
 * carries it). The write is not blocked. Nothing when there is none.
 */
export function StayWholeNotice({
  warnings,
}: {
  warnings: StayWholeWarning[];
}) {
  const { openArtifact } = useArtifactPanel();
  if (warnings.length === 0) return null;
  return (
    <Callout tone="warn" icon={Replace}>
      <p>
        <b>
          {warnings.length === 1
            ? "A plugin in this profile is taken apart but only works whole."
            : `${warnings.length} plugins in this profile are taken apart but only work whole.`}
        </b>
      </p>
      <ul aria-label="Plugins that must stay whole">
        {warnings.map((w) => (
          <li key={w.plugin}>
            <b>{w.plugin}</b>: {w.reason}{" "}
            <Button
              small
              onClick={(event) =>
                openArtifact(w.artifactId, {
                  returnFocus: event.currentTarget,
                })
              }
            >
              Replace with the whole plugin
            </Button>
          </li>
        ))}
      </ul>
    </Callout>
  );
}
