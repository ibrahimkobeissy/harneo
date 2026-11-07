import { useEffect, useState } from "react";
import type { ArtifactFit } from "../../bindings/ArtifactFit";
import { harnfitForArtifact } from "../../lib/ipc";

/**
 * The saved Harnfit verdicts of the artifacts on screen, by artifact id, for the card badges.
 * One small local query per artifact when the page renders (a page holds a few dozen); an
 * artifact already asked about is not asked again, and a failure just shows no badge.
 */
export function useArtifactFits(ids: string[]): Map<string, ArtifactFit[]> {
  const [fits, setFits] = useState<Map<string, ArtifactFit[]>>(new Map());
  const wanted = ids.join("\n");
  useEffect(() => {
    let live = true;
    const missing = wanted === "" ? [] : wanted.split("\n");
    Promise.all(
      missing.map((id) =>
        harnfitForArtifact(id).then(
          (list): [string, ArtifactFit[]] => [
            id,
            Array.isArray(list) ? list : [],
          ],
          (): [string, ArtifactFit[]] => [id, []],
        ),
      ),
    ).then((found) => {
      if (live) setFits(new Map(found));
    });
    return () => {
      live = false;
    };
  }, [wanted]);
  return fits;
}
