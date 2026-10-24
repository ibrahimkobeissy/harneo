import { useEffect, useRef, useState } from "react";
import { DetailPanel } from "../features/library";
import { useArtifactPanel } from "../lib/artifactPanel";

/**
 * Shows the artifact details panel for whichever screen asked for it (see `lib/artifactPanel`),
 * so Security, Cleanup and others open it in place instead of jumping to the Library.
 */
export function ArtifactPanelHost() {
  const { request, swapArtifact, closeArtifact, notifyChanged } =
    useArtifactPanel();
  // Artifacts left behind by following references, newest last; Back pops one.
  const [trail, setTrail] = useState<{ id: string; name: string }[]>([]);
  const origin = useRef<{ id: string; el: HTMLElement | null } | null>(null);
  const wasOpen = useRef(false);

  // Focus goes back to the row that opened the panel once it has closed.
  useEffect(() => {
    if (request) {
      wasOpen.current = true;
      if (origin.current === null || trail.length === 0) {
        origin.current = {
          id: trail[0]?.id ?? request.id,
          el: request.returnFocus,
        };
      }
      return;
    }
    if (!wasOpen.current) return;
    wasOpen.current = false;
    const from = origin.current;
    origin.current = null;
    if (!from) return;
    const el =
      from.el && from.el.isConnected
        ? from.el
        : document.querySelector<HTMLElement>(
            `[data-artifact-id="${CSS.escape(from.id)}"]`,
          );
    el?.focus();
  }, [request, trail]);

  if (!request) return null;

  function close() {
    setTrail([]);
    closeArtifact();
  }

  return (
    <DetailPanel
      id={request.id}
      focusTrust={request.focus === "trust"}
      previousName={trail.at(-1)?.name ?? null}
      onOpen={(id, fromName) => {
        setTrail((t) => [...t, { id: request.id, name: fromName }]);
        swapArtifact(id);
      }}
      onBack={() => {
        const previous = trail.at(-1);
        if (!previous) return;
        setTrail(trail.slice(0, -1));
        swapArtifact(previous.id);
      }}
      onClose={close}
      onTagsChanged={notifyChanged}
      onUpdated={notifyChanged}
      onLinksChanged={notifyChanged}
      onMembershipChanged={notifyChanged}
      onRemoved={() => {
        notifyChanged();
        close();
      }}
    />
  );
}
