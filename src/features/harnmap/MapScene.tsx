import { useId, useState, type HTMLAttributes } from "react";
import type { Zone } from "../../bindings/Zone";
import type { Placement } from "../../bindings/Placement";
import { Heartbeat } from "./Heartbeat";
import { LINE_RISE, type Layout, type Point } from "./layout";
import styles from "./MapCanvas.module.css";
import { Node } from "./Node";
import { Tooltip } from "./Tooltip";
import { zoneKey } from "./place";
import { ghostBox } from "./ghost";
import { kinds } from "../../ui";
import type { Dnd, Ghost, HookInfo, Layers, MapData } from "./types";

export const TOOLTIP_W = 248;
const HB = 20;
/** The packets that flow inside the pipe (design: `.beam` x2): dash pattern over pathLength 1000, opacity, offset. */
const BEAMS = [
  { dash: "110 140", opacity: 0.14, delay: "-3.456s" },
  { dash: "50 200", opacity: 0.32, delay: "-1.296s" },
];

function path(points: Point[]): string {
  return points.map((p, i) => `${i === 0 ? "M" : "L"} ${p.x} ${p.y}`).join(" ");
}

function stageNumber(i: number): string {
  return String(i + 1).padStart(2, "0");
}

function Rail({
  rect,
  hooks,
}: {
  rect: { x: number; y: number; w: number; h: number };
  hooks: HookInfo[];
}) {
  return (
    <div
      className={styles.rail}
      data-testid="hook-rail"
      style={{ left: rect.x, top: rect.y, width: rect.w, height: rect.h }}
    >
      {hooks.map((h) => (
        <span key={`${h.event}|${h.matcher}`} className={styles.hook}>
          hook · {h.event} · {h.matcher} · fires at every stage
        </span>
      ))}
    </div>
  );
}

/** The drawn map at scale 1: the shapes from the layout, then the nodes over them. */
export function MapScene({
  data,
  layout,
  layers,
  placements,
  selected,
  open,
  focusTip,
  onSelect,
  onOpen,
  dnd,
  ghost,
  faded = false,
}: {
  data: MapData;
  layout: Layout;
  layers: Layers;
  /** Visible placements by artifact id. */
  placements: Map<string, Placement>;
  selected: string | null;
  open: string | null;
  focusTip: boolean;
  onSelect: (id: string) => void;
  onOpen: (id: string | null, fromKeyboard: boolean) => void;
  /** Present when the map can be edited by dragging. */
  dnd?: Dnd;
  /** Harnfit: the proposed node, dashed, with red lines to what it clashes with. */
  ghost?: Ghost;
  /** Harnfit: the profile's own nodes are drawn faded and cannot be touched. */
  faded?: boolean;
}) {
  const arrow = useId();
  const [over, setOver] = useState<string | null>(null);
  /** Drag-over and drop handlers that make an element stand for `zone`. */
  const target = (zone: Zone): HTMLAttributes<HTMLElement> => ({
    onDragOver: (ev) => {
      if (!dnd?.active) return;
      ev.preventDefault();
      setOver(zoneKey(zone));
    },
    onDragLeave: () => setOver(null),
    onDrop: (ev) => {
      if (!dnd?.active) return;
      ev.preventDefault();
      setOver(null);
      dnd.onDrop(zone);
    },
  });
  const shown = layout.nodes.filter((n) =>
    n.zone.type === "stage"
      ? layers.line
      : n.zone.type === "toolbelt"
        ? layers.toolbelt
        : layers.floor,
  );
  const counts = new Map<string, number>();
  for (const n of layout.nodes) {
    if (n.zone.type === "stage") {
      counts.set(n.zone.id, (counts.get(n.zone.id) ?? 0) + 1);
    }
  }
  // The immediate next steps of the selected artifact, as Claude read them, most likely first.
  const selectedBox = shown.find((n) => n.id === selected);
  const nextIds = (
    selected ? (placements.get(selected)?.next ?? []) : []
  ).filter((id) => shown.some((n) => n.id === id));
  const nextRank = new Map(nextIds.map((id, i) => [id, i + 1]));
  const openBox = shown.find((n) => n.id === open);
  const openPlacement = open ? placements.get(open) : undefined;
  const openInfo = open ? data.artifacts[open] : undefined;
  const tipX =
    openBox && openBox.x + openBox.w + 8 + TOOLTIP_W > layout.width
      ? openBox.x - TOOLTIP_W - 8
      : openBox
        ? openBox.x + openBox.w + 8
        : 0;

  const box = ghost ? ghostBox(layout, ghost.zone) : null;
  const GhostIcon = ghost ? kinds[ghost.kind].icon : null;
  return (
    <div
      className={styles.scene}
      data-faded={faded || undefined}
      style={{ width: layout.width, height: layout.height }}
    >
      <svg
        className={styles.svg}
        width={layout.width}
        height={layout.height}
        aria-hidden="true"
      >
        <defs>
          {/* One marker per colour: WebKitGTK (Tauri on Linux) has no `context-stroke`, which
              left the arrowheads invisible. */}
          <marker
            id={arrow}
            viewBox="0 0 10 10"
            refX="8"
            refY="5"
            markerWidth="7"
            markerHeight="7"
            orient="auto-start-reverse"
          >
            <path
              d="M 1 1 L 9 5 L 1 9"
              fill="none"
              className={styles.arrowLoop}
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </marker>
          <marker
            id={`${arrow}-next`}
            viewBox="0 0 10 10"
            refX="8"
            refY="5"
            markerWidth="7"
            markerHeight="7"
            orient="auto-start-reverse"
          >
            <path
              d="M 1 1 L 9 5 L 1 9"
              fill="none"
              className={styles.arrowNext}
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </marker>
        </defs>
        {layers.line && (
          <>
            {layout.outlines.map((o, i) => (
              <path key={i} className={styles.outline} d={`${path(o)} Z`} />
            ))}
            {layout.forkJoin && (
              <g className={styles.fork}>
                {layout.forkJoin.tracks.map((t, i) => (
                  <path key={i} d={path([t.a, t.b])} />
                ))}
                <circle
                  cx={layout.forkJoin.fork.x}
                  cy={layout.forkJoin.fork.y}
                  r="3"
                />
                <circle
                  cx={layout.forkJoin.join.x}
                  cy={layout.forkJoin.join.y}
                  r="3"
                />
              </g>
            )}
            {layout.beam.length > 1 && (
              <>
                <path className={styles.pipeOuter} d={path(layout.beam)} />
                <path className={styles.pipeInner} d={path(layout.beam)} />
                {BEAMS.map((b) => (
                  <path
                    key={b.dash}
                    className={styles.beam}
                    data-testid="beam"
                    d={path(layout.beam)}
                    pathLength="1000"
                    style={{
                      strokeDasharray: b.dash,
                      opacity: b.opacity,
                      animationDelay: b.delay,
                    }}
                  />
                ))}
              </>
            )}
            {layout.beam.length > 1 &&
              layout.shape !== "cycle" &&
              (() => {
                const end = layout.beam[layout.beam.length - 1]!;
                return (
                  <path
                    className={styles.beamEnd}
                    d={`M ${end.x + 14} ${end.y - 6} L ${end.x + 22} ${end.y} L ${end.x + 14} ${end.y + 6}`}
                  />
                );
              })()}
            {layout.stages.map((s, i) => {
              const dot = layout.beam[i];
              if (!dot) return null;
              return (
                <g key={`stem-${s.id}`} data-testid="stage-dot">
                  <path
                    className={styles.stem}
                    d={`M ${dot.x} ${dot.y} L ${dot.x} ${dot.y + LINE_RISE}`}
                  />
                  <circle
                    className={styles.dot}
                    data-lit={(counts.get(s.id) ?? 0) > 0}
                    data-testid="valve"
                    cx={dot.x}
                    cy={dot.y}
                    r="5"
                  />
                </g>
              );
            })}
            {layout.returnArrow && (
              <path
                className={styles.loop}
                d={`M ${layout.returnArrow.from.x} ${layout.returnArrow.from.y} Q ${layout.returnArrow.via.x} ${layout.returnArrow.via.y} ${layout.returnArrow.to.x} ${layout.returnArrow.to.y}`}
                markerEnd={`url(#${arrow})`}
              />
            )}
            {layout.loops.map((l, i) => (
              <g key={i} className={styles.loopG}>
                <path
                  className={styles.loop}
                  data-testid="loop-path"
                  d={
                    l.curve
                      ? `M ${l.start.x} ${l.start.y} C ${l.curve.c1.x} ${l.curve.c1.y} ${l.curve.c2.x} ${l.curve.c2.y} ${l.end.x} ${l.end.y}`
                      : path(l.points)
                  }
                  markerEnd={`url(#${arrow})`}
                />
                <rect
                  x={l.labelBox.x}
                  y={l.labelBox.y}
                  width={l.labelBox.w}
                  height={l.labelBox.h}
                  rx={4}
                />
                <text
                  x={l.labelBox.x + l.labelBox.w / 2}
                  y={l.labelBox.y + 12}
                  textAnchor="middle"
                >
                  {l.label}
                </text>
              </g>
            ))}
          </>
        )}
        {ghost &&
          box &&
          ghost.conflicts.flatMap((id) => {
            const other = layout.nodes.find((n) => n.id === id);
            return other
              ? [
                  <line
                    key={id}
                    className={styles.clash}
                    data-testid="conflict-line"
                    x1={box.x + box.w / 2}
                    y1={box.y + box.h / 2}
                    x2={other.x + other.w / 2}
                    y2={other.y + other.h / 2}
                  />,
                ]
              : [];
          })}
      </svg>

      {layers.line &&
        layout.zones.map((z) => (
          <div
            key={z.name}
            className={styles.zone}
            style={{
              left: z.rect.x,
              top: z.rect.y,
              width: z.rect.w,
              height: z.rect.h,
            }}
          >
            <span>{z.name}</span>
          </div>
        ))}
      {layers.line &&
        layout.bundles.map((b) => (
          <div
            key={`${b.bundle}-${b.stage}`}
            className={styles.bnd}
            data-testid="bundle"
            style={{
              left: b.rect.x,
              top: b.rect.y,
              width: b.rect.w,
              height: b.rect.h,
            }}
          />
        ))}
      {layers.line && layout.badge && (
        <div
          className={styles.count}
          data-testid="line-badge"
          title={`${layout.nodes.length} ${data.profile ?? "stations"}`}
          aria-label={`${layout.nodes.length} ${data.profile ?? "stations"}`}
          style={{
            left: layout.badge.x,
            top: layout.badge.y,
            width: layout.badge.w,
            height: layout.badge.h,
          }}
        >
          <b>{layout.nodes.length}</b>
          <span>{data.profile?.toUpperCase() ?? "STATIONS"}</span>
        </div>
      )}
      {layers.line && layout.hookRail && (
        <Rail rect={layout.hookRail} hooks={data.hooks} />
      )}
      {layers.line &&
        layout.stages.map((s, i) => {
          const count = counts.get(s.id) ?? 0;
          const name = data.doc.stages.find((st) => st.id === s.id)?.name;
          return (
            <div
              key={s.id}
              className={styles.stage}
              data-testid="stage"
              data-stage={s.id}
              data-over={over === `stage:${s.id}`}
              {...target({ type: "stage", id: s.id })}
              style={{
                left: s.frame.x,
                top: s.frame.y,
                width: s.frame.w,
                height: s.frame.h,
              }}
            >
              <div className={styles.mh} title={name}>
                <span className={styles.n}>{stageNumber(i)}</span>
                <span className={styles.nm}>{s.label}</span>
                <span
                  className={styles.ct}
                  data-on={count > 0}
                  data-testid="light"
                >
                  {count}
                </span>
              </div>
            </div>
          );
        })}
      {layers.toolbelt && (
        <div
          className={styles.band}
          data-testid="toolbelt"
          data-over={over === "toolbelt"}
          {...target({ type: "toolbelt" })}
          style={{
            left: layout.toolbelt.x,
            top: layout.toolbelt.y,
            width: layout.toolbelt.w,
            height: layout.toolbelt.h,
          }}
        >
          <span className={styles.zlab}>
            <b>TOOLBELT</b> · any stage
          </span>
        </div>
      )}
      {layers.floor && (
        <div
          className={styles.band}
          data-testid="floor"
          data-over={over === "floor"}
          {...target({ type: "floor" })}
          style={{
            left: layout.floor.x,
            top: layout.floor.y,
            width: layout.floor.w,
            height: layout.floor.h,
          }}
        >
          <span className={styles.zlab}>
            <b>FLOOR</b> · always on
          </span>
        </div>
      )}

      {/* Above the stage frames (HTML, drawn after the first svg) and below the nodes: the frames would hide these. */}
      <svg
        className={styles.svgTop}
        width={layout.width}
        height={layout.height}
        aria-hidden="true"
      >
        {layers.line &&
          layout.links
            .filter(
              (l) =>
                shown.some((n) => n.id === l.parent) &&
                shown.some((n) => n.id === l.child),
            )
            .map((l) => (
              <path
                key={`link-${l.child}`}
                className={styles.link}
                data-testid="child-link"
                d={path(l.points)}
              />
            ))}
        {selectedBox &&
          nextIds.map((id) => {
            const to = shown.find((n) => n.id === id);
            if (!to) return null;
            const forward = to.x >= selectedBox.x + selectedBox.w;
            const a = forward
              ? {
                  x: selectedBox.x + selectedBox.w,
                  y: selectedBox.y + selectedBox.h / 2,
                }
              : {
                  x: selectedBox.x + selectedBox.w / 2,
                  y: selectedBox.y + selectedBox.h,
                };
            const b = forward
              ? { x: to.x, y: to.y + to.h / 2 }
              : { x: to.x + to.w / 2, y: to.y };
            const bend = forward
              ? { x: (a.x + b.x) / 2, y: 0 }
              : { x: 0, y: 24 };
            return (
              <path
                key={`next-${id}`}
                className={styles.nextArrow}
                data-testid="next-arrow"
                d={
                  forward
                    ? `M ${a.x} ${a.y} C ${bend.x} ${a.y} ${bend.x} ${b.y} ${b.x} ${b.y}`
                    : `M ${a.x} ${a.y} C ${a.x} ${a.y + bend.y} ${b.x} ${b.y - bend.y} ${b.x} ${b.y}`
                }
                markerEnd={`url(#${arrow}-next)`}
              />
            );
          })}
      </svg>
      {shown.map((n) => {
        const info = data.artifacts[n.id];
        const placement = placements.get(n.id);
        if (!info || !placement) return null;
        return (
          <Node
            key={n.id}
            info={info}
            placement={placement}
            box={n}
            selected={selected === n.id}
            nextRank={nextRank.get(n.id)}
            depth={n.depth}
            modelOnly={(data.facts[n.id] ?? []).some(
              (f) => f.kind.type === "modelOnly",
            )}
            onSelect={() => onSelect(n.id)}
            onInfo={() => onOpen(n.id, true)}
            drag={
              dnd && !faded
                ? { onStart: () => dnd.onStart(n.id), onEnd: dnd.onEnd }
                : undefined
            }
            dropProps={dnd ? target(n.zone) : undefined}
          />
        );
      })}
      {!faded &&
        shown.map((n) => {
          const info = data.artifacts[n.id];
          if (!info) return null;
          return (
            <Heartbeat
              key={n.id}
              name={info.name}
              x={n.x + n.w - HB - 2}
              y={n.y + n.h - HB - 2}
              open={open === n.id}
              onToggle={() => onOpen(open === n.id ? null : n.id, false)}
            />
          );
        })}
      {ghost && box && GhostIcon && (
        <div
          className={styles.ghost}
          data-testid="ghost"
          draggable={dnd ? true : undefined}
          aria-label={`${ghost.name}, new`}
          style={{ left: box.x, top: box.y, width: box.w, height: box.h }}
          onDragStart={(ev) => {
            ev.dataTransfer?.setData("text/plain", "ghost");
            dnd?.onStart("ghost");
          }}
          onDragEnd={dnd?.onEnd}
          onPointerDown={(ev) => ev.stopPropagation()}
        >
          <span className={styles.ic} aria-hidden="true">
            <GhostIcon size={12} />
          </span>
          <span className={styles.txt}>
            <b>{ghost.name}</b>
          </span>
          <span className={styles.newTag}>NEW</span>
        </div>
      )}
      {openBox && openPlacement && openInfo && (
        <Tooltip
          name={openInfo.name}
          placement={openPlacement}
          facts={data.facts[openBox.id] ?? []}
          parentName={
            openPlacement.parent
              ? (data.artifacts[openPlacement.parent]?.name ??
                openPlacement.parent)
              : undefined
          }
          alsoRunBy={(openPlacement.alsoRunBy ?? []).map(
            (id) => data.artifacts[id]?.name ?? id,
          )}
          x={tipX}
          y={openBox.y}
          focus={focusTip}
        />
      )}
    </div>
  );
}
