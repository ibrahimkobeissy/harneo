import { Maximize2, Minus, Plus } from "lucide-react";
import { useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import { kinds } from "../../ui";
import styles from "./MapCanvas.module.css";
import {
  clampZoom,
  ZOOM_MAX,
  ZOOM_MIN,
  ZOOM_STEP,
  type Layers,
  type MapViewState,
} from "./types";

const LAYER_NAMES: Array<[keyof Layers, string]> = [
  ["line", "Line"],
  ["toolbelt", "Toolbelt"],
  ["floor", "Floor"],
];

/** Layers switch, Types menu and zoom buttons. The view state is the screen's: changes go out. */
export function Toolbar({
  view,
  kindCounts,
  onView,
  onFit,
}: {
  view: MapViewState;
  /** Placed artifacts per type, for the Types menu. */
  kindCounts: Array<[ArtifactKind, number]>;
  onView: (view: MapViewState) => void;
  onFit: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const toggleKind = (kind: ArtifactKind) =>
    onView({
      ...view,
      hiddenKinds: view.hiddenKinds.includes(kind)
        ? view.hiddenKinds.filter((k) => k !== kind)
        : [...view.hiddenKinds, kind],
    });
  return (
    <div className={styles.toolbar} role="toolbar" aria-label="Map controls">
      <div className={styles.group} role="group" aria-label="Layers">
        {LAYER_NAMES.map(([key, label]) => (
          <button
            key={key}
            type="button"
            className={styles.tb}
            aria-pressed={view.layers[key]}
            onClick={() =>
              onView({
                ...view,
                layers: { ...view.layers, [key]: !view.layers[key] },
              })
            }
          >
            {label}
          </button>
        ))}
      </div>
      <div className={styles.menuWrap}>
        <button
          type="button"
          className={styles.tb}
          aria-haspopup="menu"
          aria-expanded={menu}
          onClick={() => setMenu(!menu)}
        >
          Types
          {view.hiddenKinds.length > 0 &&
            ` (${view.hiddenKinds.length} hidden)`}
        </button>
        {menu && (
          <div
            className={styles.menu}
            role="menu"
            aria-label="Types"
            onKeyDown={(ev) => ev.key === "Escape" && setMenu(false)}
          >
            {kindCounts.map(([kind, count]) => (
              <button
                key={kind}
                type="button"
                role="menuitemcheckbox"
                aria-checked={!view.hiddenKinds.includes(kind)}
                className={styles.mi}
                onClick={() => toggleKind(kind)}
              >
                <span>{kinds[kind].label}</span>
                <span className={styles.mc}>{count}</span>
              </button>
            ))}
          </div>
        )}
      </div>
      <div className={styles.group} role="group" aria-label="Zoom">
        <button
          type="button"
          className={styles.tb}
          aria-label="Zoom out"
          disabled={view.zoom <= ZOOM_MIN}
          onClick={() =>
            onView({ ...view, zoom: clampZoom(view.zoom - ZOOM_STEP) })
          }
        >
          <Minus size={14} />
        </button>
        <span className={styles.pct} aria-live="polite">
          {Math.round(view.zoom * 100)}%
        </span>
        <button
          type="button"
          className={styles.tb}
          aria-label="Zoom in"
          disabled={view.zoom >= ZOOM_MAX}
          onClick={() =>
            onView({ ...view, zoom: clampZoom(view.zoom + ZOOM_STEP) })
          }
        >
          <Plus size={14} />
        </button>
        <button type="button" className={styles.tb} onClick={onFit}>
          <Maximize2 size={13} aria-hidden="true" /> Fit
        </button>
      </div>
    </div>
  );
}
