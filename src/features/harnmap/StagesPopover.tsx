import { ArrowDown, ArrowUp, GripVertical, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Stage } from "../../bindings/Stage";
import { Button } from "../../ui";
import styles from "./Harnmap.module.css";

/** A map holds between one and this many stages. */
export const MAX_STAGES = 12;

function StageRow({
  stage,
  index,
  last,
  only,
  over,
  onRename,
  onRemove,
  onMove,
  onDragStart,
  onDragOver,
  onDrop,
}: {
  stage: Stage;
  index: number;
  last: boolean;
  only: boolean;
  over: boolean;
  onRename: (name: string) => void;
  onRemove: () => void;
  onMove: (to: number) => void;
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
}) {
  const [name, setName] = useState(stage.name);
  const [seen, setSeen] = useState(stage.name);
  if (seen !== stage.name) {
    // The saved name changed under us (a restore, a reorder): follow it.
    setSeen(stage.name);
    setName(stage.name);
  }
  const commit = () => {
    const next = name.trim();
    if (next === "") setName(stage.name);
    else if (next !== stage.name) onRename(next);
  };
  return (
    <div
      className={styles.stageRow}
      data-over={over}
      data-testid="stage-row"
      draggable
      onDragStart={(ev) => {
        ev.dataTransfer?.setData("text/plain", stage.id);
        onDragStart();
      }}
      onDragOver={(ev) => {
        ev.preventDefault();
        onDragOver();
      }}
      onDrop={(ev) => {
        ev.preventDefault();
        onDrop();
      }}
    >
      <GripVertical className={styles.grip} size={14} aria-hidden="true" />
      <span className={styles.stageNo}>
        {String(index + 1).padStart(2, "0")}
      </span>
      <input
        className={styles.stageInput}
        aria-label={`Stage ${index + 1} name`}
        value={name}
        onChange={(e) => setName(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
        }}
      />
      <Button
        small
        variant="ghost"
        aria-label={`Move ${stage.name} up`}
        disabled={index === 0}
        onClick={() => onMove(index - 1)}
      >
        <ArrowUp size={12} aria-hidden="true" />
      </Button>
      <Button
        small
        variant="ghost"
        aria-label={`Move ${stage.name} down`}
        disabled={last}
        onClick={() => onMove(index + 1)}
      >
        <ArrowDown size={12} aria-hidden="true" />
      </Button>
      <Button
        small
        variant="ghost"
        aria-label={`Remove ${stage.name}`}
        disabled={only}
        onClick={onRemove}
      >
        <X size={12} aria-hidden="true" />
      </Button>
    </div>
  );
}

/** The Stages popover: rename, reorder (drag, or the arrows), add and remove. */
export function StagesPopover({
  stages,
  onRename,
  onRemove,
  onMove,
  onAdd,
  onClose,
}: {
  stages: Stage[];
  onRename: (id: string, name: string) => void;
  onRemove: (id: string) => void;
  onMove: (id: string, to: number) => void;
  onAdd: () => void;
  onClose: () => void;
}) {
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<number | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    box.current?.focus();
  }, []);
  return (
    <div
      ref={box}
      tabIndex={-1}
      className={`${styles.pop} ${styles.stagesPop}`}
      role="dialog"
      aria-label="Edit stages"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div className={styles.panelHead}>
        <b className={styles.h4}>Stages</b>
        <Button
          small
          variant="ghost"
          aria-label="Close stages"
          onClick={onClose}
        >
          <X size={14} aria-hidden="true" />
        </Button>
      </div>
      {stages.map((stage, i) => (
        <StageRow
          key={stage.id}
          stage={stage}
          index={i}
          last={i === stages.length - 1}
          only={stages.length === 1}
          over={over === i && dragging !== null && dragging !== stage.id}
          onRename={(name) => onRename(stage.id, name)}
          onRemove={() => onRemove(stage.id)}
          onMove={(to) => onMove(stage.id, to)}
          onDragStart={() => setDragging(stage.id)}
          onDragOver={() => setOver(i)}
          onDrop={() => {
            if (dragging && dragging !== stage.id) onMove(dragging, i);
            setDragging(null);
            setOver(null);
          }}
        />
      ))}
      <Button small disabled={stages.length >= MAX_STAGES} onClick={onAdd}>
        <Plus size={12} aria-hidden="true" /> Add stage
      </Button>
      <span className={styles.muted}>
        Drag to reorder. Removing a stage sends its stations to Unplaced,
        unpinned, and removes the loops attached to it. Stages are kept by id,
        so a rename survives a re-draw.
      </span>
    </div>
  );
}
