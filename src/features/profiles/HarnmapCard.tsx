import { useEffect, useState } from "react";
import { Link } from "react-router";
import type { MapView } from "../../bindings/MapView";
import { harnmapGet } from "../../lib/ipc";
import { Badge, Panel, type BadgeTone } from "../../ui";
import styles from "./Builder.module.css";

const W = 300;
const GAP = 6;

/** A static sketch of the line: one box per stage on a pipe, as many as the map has. */
function MiniMap({ stages, placed }: { stages: number; placed: boolean }) {
  const count = Math.max(1, Math.min(stages, 12));
  const w = (W - GAP * (count - 1)) / count;
  return (
    <svg
      viewBox={`0 0 ${W} 56`}
      width="100%"
      height="56"
      aria-hidden="true"
      className={styles.mini}
      data-testid="mini-map"
    >
      <path d={`M 2 8 H ${W - 2}`} className={styles.miniPipe} />
      {Array.from({ length: count }, (_, i) => (
        <g key={i} className={styles.miniStage} data-placed={placed}>
          <rect x={i * (w + GAP)} y="16" width={w} height="34" rx="5" />
          <circle cx={i * (w + GAP) + w - 6} cy="22" r="1.6" />
        </g>
      ))}
    </svg>
  );
}

function state(view: MapView): { label: string; tone: BadgeTone } {
  if (!view.drawn) return { label: "Not drawn yet", tone: "neutral" };
  if (view.stale) return { label: "Out of date", tone: "warn" };
  return { label: "Up to date", tone: "ok" };
}

function drawn(createdAt: number): string {
  const d = new Date(createdAt * 1000);
  return `${d.toLocaleDateString(undefined, { day: "numeric", month: "short" })}, ${d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" })}`;
}

/**
 * The profile builder's Harnmap card: a mini preview, the state badge, when it was drawn and how
 * much of the profile is placed, and the way into the Harnmap screen. It only reads.
 * Pass a `version` that changes when the profile is saved, so the badge follows.
 */
export function HarnmapCard({
  profile,
  version,
}: {
  profile: bigint;
  version: number;
}) {
  const [view, setView] = useState<MapView | null>(null);

  useEffect(() => {
    let live = true;
    harnmapGet(profile).then(
      (next) => live && setView(next ?? null),
      () => live && setView(null),
    );
    return () => {
      live = false;
    };
  }, [profile, version]);

  if (!view) return null;
  const { label, tone } = state(view);
  const total = Object.keys(view.artifacts).length;
  const placed = view.doc.placements.filter(
    (p) => view.artifacts[p.artifact],
  ).length;
  const href = `/profiles/${profile}/harnmap`;
  return (
    <Panel
      title="Harnmap"
      titleId="harnmap-card"
      aside={<Badge tone={tone}>{label}</Badge>}
    >
      <Link to={href} className={styles.miniLink} aria-label="Open the Harnmap">
        <MiniMap stages={view.doc.stages.length} placed={view.drawn} />
      </Link>
      <p className={styles.drawLine}>
        {view.drawn
          ? `Drawn ${drawn(view.createdAt)} · ${view.drawCount} ${view.drawCount === 1 ? "drawing" : "drawings"} in history · ${placed} of ${total} artifacts placed`
          : "Not drawn yet. Draw it to see this profile as a factory."}
      </p>
      <Link to={href} className={styles.harnmap}>
        Open Harnmap
      </Link>
    </Panel>
  );
}
