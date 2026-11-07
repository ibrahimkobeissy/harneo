import { Pin } from "lucide-react";
import type { HTMLAttributes } from "react";
import type { Placement } from "../../bindings/Placement";
import { kinds } from "../../ui";
import styles from "./MapCanvas.module.css";
import type { ArtifactInfo } from "./types";

/** One artifact on the map: type icon, name, note, pin mark; `i` opens its tooltip. */
export function Node({
  info,
  placement,
  box,
  selected,
  onSelect,
  onInfo,
  drag,
  dropProps,
}: {
  info: ArtifactInfo;
  placement: Placement;
  box: { x: number; y: number; w: number; h: number };
  selected: boolean;
  onSelect: () => void;
  onInfo: () => void;
  /** Present when the node can be dragged to another zone. */
  drag?: { onStart: () => void; onEnd: () => void };
  /** Drag-over and drop handlers: dropping on a node is dropping on its zone. */
  dropProps?: HTMLAttributes<HTMLElement>;
}) {
  const meta = kinds[info.kind];
  const Icon = meta.icon;
  return (
    <button
      type="button"
      className={styles.nd}
      data-selected={selected}
      data-node={placement.artifact}
      aria-pressed={selected}
      aria-keyshortcuts="i"
      aria-label={`${info.name}, ${meta.label}${placement.pinned ? ", pinned" : ""}`}
      style={{ left: box.x, top: box.y, width: box.w, height: box.h }}
      onClick={(ev) => {
        ev.stopPropagation();
        onSelect();
      }}
      onPointerDown={(ev) => ev.stopPropagation()}
      draggable={drag ? true : undefined}
      onDragStart={(ev) => {
        ev.dataTransfer?.setData("text/plain", placement.artifact);
        drag?.onStart();
      }}
      onDragEnd={drag?.onEnd}
      {...dropProps}
      onKeyDown={(ev) => {
        if (ev.key === "i" && !ev.ctrlKey && !ev.metaKey && !ev.altKey) {
          ev.preventDefault();
          onInfo();
        }
      }}
    >
      <span className={styles.ic} data-tint={meta.tint} aria-hidden="true">
        <Icon size={12} />
      </span>
      <span className={styles.txt}>
        <b>
          {info.name}
          {placement.pinned && (
            <Pin
              className={styles.pin}
              size={10}
              role="img"
              aria-label="pinned"
            />
          )}
        </b>
        <i>{placement.note}</i>
      </span>
    </button>
  );
}
