import { useEffect, useId, useRef, useState } from "react";
import type { MapDoc } from "../../bindings/MapDoc";
import type { Zone } from "../../bindings/Zone";
import { Button } from "../../ui";
import styles from "./Harnmap.module.css";
import { zoneKey, zoneName, zoneOfKey } from "./place";

/** The "Why here?" popover: where an artifact goes and one line that becomes its note. */
export function WhyHere({
  name,
  doc,
  zone,
  onPlace,
  onCancel,
}: {
  name: string;
  doc: MapDoc;
  zone: Zone;
  onPlace: (zone: Zone, note: string) => void;
  onCancel: () => void;
}) {
  const [chosen, setChosen] = useState(zoneKey(zone));
  const [note, setNote] = useState("");
  const field = useRef<HTMLInputElement>(null);
  const noteId = useId();
  const zoneId = useId();
  useEffect(() => {
    field.current?.focus();
  }, []);
  const target = zoneOfKey(chosen);
  const zones: Zone[] = [
    ...doc.stages.map((s): Zone => ({ type: "stage", id: s.id })),
    { type: "toolbelt" },
    { type: "floor" },
  ];
  return (
    <div
      className={`${styles.pop} ${styles.whyPop}`}
      role="dialog"
      aria-label="Why here?"
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <b className={styles.label}>
        Place <code>{name}</code> in {zoneName(doc, target)}
      </b>
      <label className={styles.label} htmlFor={zoneId}>
        Where
      </label>
      <select
        id={zoneId}
        className={styles.select}
        value={chosen}
        onChange={(e) => setChosen(e.target.value)}
      >
        {zones.map((z) => (
          <option key={zoneKey(z)} value={zoneKey(z)}>
            {zoneName(doc, z)}
          </option>
        ))}
      </select>
      <label className={styles.label} htmlFor={noteId}>
        Why here?{" "}
        <span className={styles.muted}>One line, becomes its note.</span>
      </label>
      <input
        id={noteId}
        ref={field}
        className={styles.search}
        maxLength={60}
        value={note}
        onChange={(e) => setNote(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onPlace(target, note.trim());
        }}
      />
      <div className={styles.row}>
        <Button small variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          small
          variant="primary"
          onClick={() => onPlace(target, note.trim())}
        >
          Place
        </Button>
      </div>
    </div>
  );
}
