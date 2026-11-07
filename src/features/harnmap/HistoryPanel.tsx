import { X } from "lucide-react";
import type { DrawSummary } from "../../bindings/DrawSummary";
import { Badge, Button } from "../../ui";
import { modelLabel } from "./draw";
import styles from "./Harnmap.module.css";
import { stamp } from "./place";

/** Map history: every draw with its date, model, description and change line. */
export function HistoryPanel({
  draws,
  previewing,
  busy,
  onPreview,
  onRestore,
  onClose,
}: {
  /** Newest first; `null` while loading. */
  draws: DrawSummary[] | null;
  previewing: number | null;
  busy: boolean;
  onPreview: (no: number) => void;
  onRestore: (no: number) => void;
  onClose: () => void;
}) {
  return (
    <>
      <div className={styles.sect}>
        <div className={styles.panelHead}>
          <h2 className={styles.h4}>Map history</h2>
          <Button
            small
            variant="ghost"
            aria-label="Close history"
            onClick={onClose}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </div>
        <p className={styles.p}>
          Every draw is kept. A re-draw never overwrites; restoring makes a copy
          on top.
        </p>
      </div>
      <div className={styles.sect}>
        {draws === null && <p className={styles.muted}>Loading the history…</p>}
        {draws?.map((d, i) => (
          <div
            key={d.no}
            className={styles.hist}
            data-current={i === 0}
            aria-label={`Draw ${d.no}`}
            role="group"
          >
            <div className={styles.histHead}>
              <b>Draw {d.no}</b>
              <span className={styles.mono}>
                {stamp(d.createdAt)} · {modelLabel(d.model)}
              </span>
            </div>
            {d.description && <p className={styles.p}>“{d.description}”</p>}
            <p className={styles.muted}>{d.changeLine}</p>
            <div className={styles.row}>
              {i === 0 ? (
                <Badge>Current</Badge>
              ) : (
                <>
                  <Button
                    small
                    aria-pressed={previewing === d.no}
                    onClick={() => onPreview(d.no)}
                  >
                    Preview
                  </Button>
                  <Button
                    small
                    variant="ghost"
                    disabled={busy}
                    onClick={() => onRestore(d.no)}
                  >
                    Restore
                  </Button>
                </>
              )}
            </div>
          </div>
        ))}
        <p className={styles.muted}>
          History goes with the profile: export, import and clone keep it.
        </p>
      </div>
    </>
  );
}
