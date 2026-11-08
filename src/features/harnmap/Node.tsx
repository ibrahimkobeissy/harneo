import { Bot, Pin } from "lucide-react";
import type { HTMLAttributes } from "react";
import type { Placement } from "../../bindings/Placement";
import { kinds } from "../../ui";
import styles from "./MapCanvas.module.css";
import type { ArtifactInfo } from "./types";

/**
 * The note without the artifact's own name in front ("/spec — writes the spec" → "writes the
 * spec"), so the one line a note has says what it does here.
 */
export function noteText(name: string, note: string): string {
  const bare = name.replace(/^\//, "");
  const head = new RegExp(
    `^/?${bare.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*(?:[—–:-]|·)\\s*`,
    "i",
  );
  const rest = note.replace(head, "");
  if (rest === note || rest.length === 0) return note;
  return rest.charAt(0).toUpperCase() + rest.slice(1);
}

/** One artifact on the map: type icon, name, note, pin mark; `i` opens its tooltip. */
export function Node({
  info,
  placement,
  box,
  selected,
  nextRank,
  modelOnly = false,
  depth = 0,
  onSelect,
  onInfo,
  drag,
  dropProps,
}: {
  info: ArtifactInfo;
  placement: Placement;
  box: { x: number; y: number; w: number; h: number };
  selected: boolean;
  /** Set when this artifact is a natural next step of the selected one: 1 is the most likely. */
  nextRank?: number;
  /** The skill is never started by the user (`user-invocable: false`): a quiet mark shows it. */
  modelOnly?: boolean;
  /** 1 or 2 for a child drawn under its parent. */
  depth?: number;
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
      data-next={nextRank !== undefined || undefined}
      data-node={placement.artifact}
      data-depth={depth > 0 ? depth : undefined}
      aria-pressed={selected}
      aria-keyshortcuts="i"
      aria-label={`${info.name}, ${meta.label}${placement.pinned ? ", pinned" : ""}${modelOnly ? ", model only" : ""}${nextRank !== undefined ? `, next step ${nextRank}` : ""}`}
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
      {nextRank !== undefined && (
        <span className={styles.nx} aria-hidden="true">
          {nextRank}
        </span>
      )}
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
          {modelOnly && (
            <Bot
              className={styles.pin}
              size={10}
              role="img"
              aria-label="model only"
            />
          )}
        </b>
        <i>{noteText(info.name, placement.note)}</i>
      </span>
    </button>
  );
}
