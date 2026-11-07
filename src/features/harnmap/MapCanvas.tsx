import { useEffect, useMemo, useRef, useState } from "react";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Placement } from "../../bindings/Placement";
import { layout } from "./layout";
import styles from "./MapCanvas.module.css";
import { MapScene } from "./MapScene";
import { Toolbar } from "./Toolbar";
import {
  clampZoom,
  QUIET_BELOW,
  ZOOM_STEP,
  type MapData,
  type MapViewState,
} from "./types";

const FIT_MARGIN = 24;
/** Size assumed for Fit when the container has not been measured. */
const FALLBACK = { w: 1000, h: 600 };

interface Pan {
  x: number;
  y: number;
}

function Legend() {
  return (
    <div className={styles.legend} aria-label="Legend">
      <span>
        <i className={styles.lb} /> Bundle · from the files
      </span>
      <span>
        <i className={styles.lf} /> From the files
      </span>
      <span>
        <i className={styles.lc} /> Claude’s reading
      </span>
    </div>
  );
}

/**
 * The map: stages, loops, the hook rail, bundles, toolbelt and floor, with their nodes.
 * Presentational: the screen owns the data, the selection and the view state.
 * Zoom 40–150 % (buttons, Ctrl+wheel), drag the background to pan, Fit; below 75 % plain boxes.
 */
export function MapCanvas({
  data,
  selected,
  view,
  onSelect,
  onView,
}: {
  data: MapData;
  selected: string | null;
  view: MapViewState;
  onSelect: (id: string | null) => void;
  onView: (view: MapViewState) => void;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  const [pan, setPan] = useState<Pan>({ x: 0, y: 0 });
  const [open, setOpen] = useState<{ id: string; keyboard: boolean } | null>(
    null,
  );
  const [paused, setPaused] = useState(
    () => document.visibilityState === "hidden",
  );
  const drag = useRef<{
    x: number;
    y: number;
    pan: Pan;
    moved: boolean;
  } | null>(null);

  const { visible, placements, kindCounts } = useMemo(() => {
    const placed = data.doc.placements.flatMap((p) => {
      const info = data.artifacts[p.artifact];
      return info ? [{ p, kind: info.kind }] : [];
    });
    const counts = new Map<ArtifactKind, number>();
    for (const { kind } of placed)
      counts.set(kind, (counts.get(kind) ?? 0) + 1);
    const shown = placed
      .filter(
        ({ p, kind }) =>
          !view.hidden.includes(p.artifact) && !view.hiddenKinds.includes(kind),
      )
      .map(({ p }) => p);
    return {
      visible: shown,
      placements: new Map<string, Placement>(shown.map((p) => [p.artifact, p])),
      kindCounts: [...counts.entries()],
    };
  }, [data, view.hidden, view.hiddenKinds]);

  const lay = useMemo(
    () =>
      layout({
        shape: data.doc.shape,
        stages: data.doc.stages,
        loops: data.doc.loops,
        phases: data.doc.phases,
        parallel: data.doc.parallel,
        placements: visible,
        bundles: data.bundles,
        hasHooks: data.hooks.length > 0,
      }),
    [data, visible],
  );

  // Latest values for the native wheel listener, which is attached once.
  const latest = useRef({ view, pan, onView });
  useEffect(() => {
    latest.current = { view, pan, onView };
  });

  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    const onWheel = (ev: WheelEvent) => {
      if (!ev.ctrlKey) return;
      ev.preventDefault();
      const { view: v, pan: p, onView: change } = latest.current;
      const zoom = clampZoom(v.zoom + (ev.deltaY < 0 ? ZOOM_STEP : -ZOOM_STEP));
      if (zoom === v.zoom) return;
      const box = el.getBoundingClientRect();
      const px = ev.clientX - box.left;
      const py = ev.clientY - box.top;
      const k = zoom / v.zoom;
      setPan({
        x: Math.round(px - (px - p.x) * k),
        y: Math.round(py - (py - p.y) * k),
      });
      change({ ...v, zoom });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  // Pause the animations while the window is hidden.
  useEffect(() => {
    const onChange = () => setPaused(document.visibilityState === "hidden");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, []);

  // Escape closes the open tooltip and puts focus back on its node.
  const openId = open?.id;
  useEffect(() => {
    if (!openId) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key !== "Escape") return;
      setOpen(null);
      viewport.current
        ?.querySelector<HTMLElement>(`[data-node="${CSS.escape(openId)}"]`)
        ?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [openId]);

  function fit() {
    const w = viewport.current?.clientWidth || FALLBACK.w;
    const h = viewport.current?.clientHeight || FALLBACK.h;
    const zoom = clampZoom(
      Math.min(
        (w - 2 * FIT_MARGIN) / lay.width,
        (h - 2 * FIT_MARGIN) / lay.height,
      ),
    );
    setPan({
      x: Math.round((w - lay.width * zoom) / 2),
      y: Math.round((h - lay.height * zoom) / 2),
    });
    onView({ ...view, zoom });
  }

  const quiet = view.zoom < QUIET_BELOW;
  return (
    <div className={styles.canvas} data-paused={paused}>
      <Toolbar
        view={view}
        kindCounts={kindCounts}
        onView={onView}
        onFit={fit}
      />
      <div
        ref={viewport}
        className={styles.viewport}
        data-testid="viewport"
        data-quiet={quiet}
        onPointerDown={(ev) => {
          drag.current = {
            x: ev.clientX,
            y: ev.clientY,
            pan,
            moved: false,
          };
          ev.currentTarget.setPointerCapture?.(ev.pointerId);
        }}
        onPointerMove={(ev) => {
          const d = drag.current;
          if (!d) return;
          const dx = ev.clientX - d.x;
          const dy = ev.clientY - d.y;
          if (Math.abs(dx) + Math.abs(dy) > 3) d.moved = true;
          setPan({ x: d.pan.x + dx, y: d.pan.y + dy });
        }}
        onPointerUp={() => {
          const d = drag.current;
          drag.current = null;
          if (d && !d.moved) {
            setOpen(null);
            onSelect(null);
          }
        }}
      >
        <div
          className={styles.world}
          style={{
            transform: `translate(${pan.x}px, ${pan.y}px) scale(${view.zoom})`,
          }}
        >
          <MapScene
            data={data}
            layout={lay}
            layers={view.layers}
            placements={placements}
            selected={selected}
            open={open?.id ?? null}
            focusTip={open?.keyboard ?? false}
            onSelect={onSelect}
            onOpen={(id, keyboard) => setOpen(id ? { id, keyboard } : null)}
          />
        </div>
      </div>
      <Legend />
    </div>
  );
}
