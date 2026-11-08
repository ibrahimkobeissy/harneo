import { EyeOff, Pin, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Fact } from "../../bindings/Fact";
import type { MapDoc } from "../../bindings/MapDoc";
import type { Placement } from "../../bindings/Placement";
import type { Zone } from "../../bindings/Zone";
import { libraryDetail } from "../../lib/ipc";
import { Badge, Button, kinds } from "../../ui";
import styles from "./Harnmap.module.css";
import { zoneKey, zoneName, zoneOfKey } from "./place";
import type { ArtifactInfo } from "./types";

/** The note autosaves this long after the last keystroke. */
export const NOTE_DELAY_MS = 500;
/** The limit the core cuts a note to. */
const NOTE_MAX = 60;

/**
 * What the side panel shows for a selected node: why it is here (Claude or you), the facts from
 * the files, the description, the note (saved as you type), Move to, Remove, Hide and Unpin.
 * Key it by artifact so a switch flushes the pending note.
 */
export function NodeDetail({
  id,
  info,
  placement,
  facts,
  doc,
  parentName,
  alsoRunBy = [],
  onDetach,
  onClose,
  onNote,
  onMove,
  onRemove,
  onHide,
  onUnpin,
}: {
  id: string;
  info: ArtifactInfo;
  placement: Placement;
  facts: Fact[];
  doc: MapDoc;
  /** Display name of the artifact this runs only through, when it has a parent. */
  parentName?: string;
  /** Display names of the other artifacts that also run it. */
  alsoRunBy?: string[];
  onDetach: () => void;
  onClose: () => void;
  onNote: (text: string) => void;
  onMove: (zone: Zone) => void;
  onRemove: () => void;
  onHide: () => void;
  onUnpin: () => void;
}) {
  const meta = kinds[info.kind];
  const Icon = meta.icon;
  const [note, setNote] = useState(placement.note);
  const [description, setDescription] = useState<string | null>(null);
  // The latest text, and whether it still has to be sent.
  const pending = useRef<string | null>(null);
  const send = useRef(onNote);
  useEffect(() => {
    send.current = onNote;
  });

  useEffect(() => {
    let live = true;
    libraryDetail(id).then(
      (d) => live && setDescription(d?.artifact?.description ?? null),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [id]);

  // Send a note once typing pauses; a pending one is sent when the panel goes away.
  useEffect(() => {
    if (note === placement.note) {
      pending.current = null;
      return;
    }
    pending.current = note;
    const timer = setTimeout(() => {
      pending.current = null;
      send.current(note);
    }, NOTE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [note, placement.note]);
  useEffect(
    () => () => {
      if (pending.current !== null) send.current(pending.current);
    },
    [],
  );

  const byYou = placement.by === "user";
  const zones: Zone[] = [
    ...doc.stages.map((s): Zone => ({ type: "stage", id: s.id })),
    { type: "toolbelt" },
    { type: "floor" },
  ];
  return (
    <>
      <div className={styles.sect}>
        <div className={styles.panelHead}>
          <div>
            <h2 className={`${styles.h4} ${styles.mono}`}>
              <Icon size={14} aria-hidden="true" /> {info.name}
            </h2>
            <div className={styles.muted}>
              {meta.label} · {zoneName(doc, placement.zone)}
            </div>
          </div>
          <Button
            small
            variant="ghost"
            aria-label="Close details"
            onClick={onClose}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </div>
        {placement.pinned && (
          <div className={styles.listRow}>
            <Badge>
              <Pin size={10} aria-hidden="true" /> Pinned · stays on re-draw
            </Badge>
            <Button small variant="ghost" onClick={onUnpin}>
              Unpin, let Claude place it
            </Button>
          </div>
        )}
      </div>
      {parentName !== undefined && (
        <div className={styles.sect}>
          <div className={styles.panelHead}>
            <h3 className={styles.eyebrow}>RUNS THROUGH</h3>
            <span className={styles.src}>Claude’s reading</span>
          </div>
          <p className={styles.p}>Runs only through {parentName}.</p>
          {alsoRunBy.length > 0 && (
            <p className={styles.p}>Also run by {alsoRunBy.join(", ")}.</p>
          )}
          <div className={styles.row}>
            <Button onClick={onDetach}>Detach</Button>
          </div>
          <span className={styles.muted}>
            Draws it as an ordinary node and pins it. Dragging it to another
            zone does the same.
          </span>
        </div>
      )}
      <div className={styles.sect}>
        <div className={styles.panelHead}>
          <h3 className={styles.eyebrow}>
            {byYou ? "WHY HERE" : "WHY CLAUDE PUT IT HERE"}
          </h3>
          <span className={styles.src}>
            {byYou ? "You" : "Claude’s reading"}
          </span>
        </div>
        <p className={styles.p}>
          {placement.why ||
            (byYou ? "You placed it here by hand." : "No reason was given.")}
        </p>
      </div>
      <div className={styles.sect}>
        <div className={styles.panelHead}>
          <h3 className={styles.eyebrow}>FROM THE FILES</h3>
          <span className={styles.src}>facts</span>
        </div>
        {facts.length > 0 ? (
          <ul className={styles.facts2}>
            {facts.map((f) => (
              <li key={f.text}>{f.text}</li>
            ))}
          </ul>
        ) : (
          <p className={styles.muted}>Nothing special in its files.</p>
        )}
      </div>
      <div className={styles.sect}>
        <h3 className={styles.eyebrow}>DESCRIPTION</h3>
        <p className={styles.p}>{description ?? "No description."}</p>
      </div>
      <div className={styles.sect}>
        <label className={styles.eyebrow} htmlFor="node-note">
          NOTE ON THE MAP
        </label>
        <textarea
          id="node-note"
          className={styles.ta}
          maxLength={NOTE_MAX}
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
        <span className={styles.muted}>
          Shown under the name. Saved as you type.
        </span>
      </div>
      <div className={styles.sect}>
        <label className={styles.eyebrow} htmlFor="node-move">
          MOVE TO
        </label>
        <select
          id="node-move"
          className={styles.select}
          value={zoneKey(placement.zone)}
          onChange={(e) => onMove(zoneOfKey(e.target.value))}
        >
          {zones.map((z) => (
            <option key={zoneKey(z)} value={zoneKey(z)}>
              {zoneName(doc, z)}
              {zoneKey(z) === zoneKey(placement.zone) ? " (current)" : ""}
            </option>
          ))}
        </select>
        <span className={styles.muted}>
          Or drag it on the map. Moving pins it.
        </span>
        <div className={styles.row}>
          <Button onClick={onRemove}>Remove from map</Button>
          <Button variant="ghost" onClick={onHide}>
            <EyeOff size={14} aria-hidden="true" /> Hide
          </Button>
        </div>
      </div>
    </>
  );
}
