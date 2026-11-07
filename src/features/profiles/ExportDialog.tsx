import { useEffect, useState } from "react";
import type { DrawSummary } from "../../bindings/DrawSummary";
import type { FitSummary } from "../../bindings/FitSummary";
import type { MapView } from "../../bindings/MapView";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { harnfitList, harnmapGet, harnmapHistory } from "../../lib/ipc";
import { Dialog } from "./Dialog";
import styles from "./Profiles.module.css";

/** What the file will carry besides the profile and the current map. */
export interface ExportChoice {
  includeHistory: boolean;
  includeFits: boolean;
}

interface Extras {
  map: MapView | null;
  history: DrawSummary[];
  fits: FitSummary[];
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

function Row({
  title,
  sub,
  toggle,
}: {
  title: string;
  sub: string;
  toggle?: {
    label: string;
    on: boolean;
    disabled: boolean;
    onChange: (on: boolean) => void;
  };
}) {
  return (
    <li className={styles.xrow}>
      <span className={styles.xtext}>
        <span className={styles.xtitle}>{title}</span>
        <span className={styles.xsub} title={sub}>
          {sub}
        </span>
      </span>
      {toggle && (
        <button
          type="button"
          role="switch"
          aria-checked={toggle.on}
          aria-label={toggle.label}
          disabled={toggle.disabled}
          className={styles.switch}
          onClick={() => toggle.onChange(!toggle.on)}
        >
          <i />
        </button>
      )}
    </li>
  );
}

/**
 * Asks what the profile file carries before the save dialog: the artifacts always, the current
 * map when there is one, its earlier drawings and the saved fits when switched on. The counts
 * come from the Harnmap commands; if they fail the file simply has no map rows.
 */
export function ExportDialog({
  profile,
  fileName,
  onExport,
  onClose,
}: {
  profile: ProfileSummary;
  fileName: string;
  onExport: (choice: ExportChoice) => Promise<void>;
  onClose: () => void;
}) {
  const [extras, setExtras] = useState<Extras | null>(null);
  const [history, setHistory] = useState(true);
  const [fits, setFits] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    void Promise.all([
      harnmapGet(profile.id).catch(() => null),
      harnmapHistory(profile.id).catch(() => []),
      harnfitList(profile.id).catch(() => []),
    ]).then(([map, past, saved]) => {
      if (live) setExtras({ map, history: past, fits: saved });
    });
    return () => {
      live = false;
    };
  }, [profile.id]);

  const artifacts = profile.counts.reduce((n, c) => n + c.count, 0);
  const drawn = extras?.map?.drawn === true;
  const earlier = Math.max(0, (extras?.history.length ?? 0) - 1);
  const saved = extras?.fits ?? [];

  async function submit() {
    setBusy(true);
    try {
      // Nothing to include when there is nothing: the switches only turn things off, so a
      // failed count never costs the file its history or fits.
      await onExport({ includeHistory: history, includeFits: fits });
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      title={`Export ${profile.name}`}
      sub="Nothing is lost: every drawing costs tokens and time, so the map travels with the profile."
      submit="Export"
      busy={busy || extras === null}
      onSubmit={() => void submit()}
      onClose={onClose}
    >
      <ul className={styles.xrows} aria-label="What the file carries">
        <Row
          title={`${plural(artifacts, "artifact", "artifacts")} and their bundles`}
          sub={plural(profile.fileCount, "file", "files")}
        />
        <Row
          title={
            extras === null
              ? "Harnmap"
              : drawn
                ? `Harnmap, draw ${extras.map?.no ?? 0}`
                : "No map drawn yet"
          }
          sub={
            drawn
              ? "Stages, placements, notes, pins, hidden nodes"
              : "Nothing to carry, the profile alone travels"
          }
        />
        <Row
          title={`Map history, ${plural(earlier, "earlier drawing", "earlier drawings")}`}
          sub="With the description each one used"
          toggle={{
            label: "Include map history",
            on: history && drawn && earlier > 0,
            disabled: !drawn || earlier === 0,
            onChange: setHistory,
          }}
        />
        <Row
          title={`Saved fits, ${saved.length}`}
          sub={saved.map((f) => f.candidate.label).join(", ") || "None yet"}
          toggle={{
            label: "Include saved fits",
            on: fits && saved.length > 0,
            disabled: saved.length === 0,
            onChange: setFits,
          }}
        />
      </ul>
      <p className={styles.xfile}>{fileName} · format v2</p>
    </Dialog>
  );
}
