import type { Zone } from "../../bindings/Zone";
import {
  BAND_PAD,
  FRAME_PAD,
  GAP,
  HEADER_H,
  NODE_H,
  NODE_W,
  type Layout,
  type Rect,
} from "./layout";

/**
 * Where the dashed NEW node sits in `zone`: after the zone's last node (a stage column goes
 * down, the toolbelt and the floor fill rows left to right), or at the zone's start when it is
 * empty. A stage that is not on the map falls back to the floor.
 */
export function ghostBox(layout: Layout, zone: Zone): Rect {
  const w = NODE_W;
  const h = NODE_H;
  const frame =
    zone.type === "stage"
      ? layout.stages.find((s) => s.id === zone.id)?.frame
      : undefined;
  const band = zone.type === "toolbelt" ? layout.toolbelt : layout.floor;
  const there = layout.nodes.filter(
    (n) =>
      n.zone.type === zone.type &&
      (zone.type !== "stage" ||
        (n.zone.type === "stage" && n.zone.id === zone.id)),
  );
  const last = there.at(-1);
  if (frame) {
    return {
      x: frame.x + FRAME_PAD,
      y: last ? last.y + last.h + GAP : frame.y + HEADER_H + GAP,
      w,
      h,
    };
  }
  if (!last) return { x: band.x + BAND_PAD, y: band.y + BAND_PAD, w, h };
  const x = last.x + last.w + GAP;
  return x + w <= band.x + band.w - BAND_PAD
    ? { x, y: last.y, w, h }
    : { x: band.x + BAND_PAD, y: last.y + last.h + GAP, w, h };
}
