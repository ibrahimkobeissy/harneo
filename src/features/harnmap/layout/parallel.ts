import {
  pick,
  assemble,
  frameWidth,
  GAP,
  phaseZones,
  size,
  ZONE_LABEL_H,
  type Layout,
  type LayoutInput,
  type Point,
} from "./common";

const TRACK_GAP = 16;

/**
 * Stages left to right. The stages in `parallel` share one column, stacked as tracks between a fork
 * and a join; phases are zones behind their stages.
 */
export function layoutParallel(input: LayoutInput): Layout {
  const pitch = frameWidth(size(input).w) + GAP;
  const isTrack = input.stages.map((s) => input.parallel.includes(s.id));
  const firstTrack = isTrack.indexOf(true);
  return assemble(
    input,
    (heights) => {
      let column = 0;
      let trackY = 0;
      return heights.map((h, i) => {
        if (!isTrack[i]) return { x: column++ * pitch, y: 0 };
        if (i === firstTrack) column++;
        const p: Point = { x: (column - 1) * pitch, y: trackY };
        trackY += h + TRACK_GAP;
        return p;
      });
    },
    (frames, ctx) => {
      const tracks = frames.filter((_, i) => isTrack[i]);
      const zones = phaseZones(input.phases, frames, ctx);
      if (tracks.length === 0) return { zones };
      const first = pick(tracks, 0).frame;
      const x = first.x;
      const y = ctx.top + 18;
      return {
        zones,
        forkJoin: {
          fork: { x: x - GAP / 2, y },
          join: { x: x + first.w + GAP / 2, y },
          tracks: tracks.map((t) => ({
            a: { x: t.frame.x, y: t.header.y + t.header.h / 2 },
            b: { x: t.frame.x + t.frame.w, y: t.header.y + t.header.h / 2 },
          })),
        },
      };
    },
    input.phases.length > 0 ? ZONE_LABEL_H : 0,
  );
}
