import {
  assemble,
  frameWidth,
  GAP,
  size,
  type Layout,
  type LayoutInput,
} from "./common";

/** Stages left to right on a 144 px pitch, stations stacked under each, loops as arcs below. */
export function layoutLine(input: LayoutInput): Layout {
  const pitch = frameWidth(size(input).w) + GAP;
  return assemble(
    input,
    (heights) => heights.map((_, i) => ({ x: i * pitch, y: 0 })),
    () => ({}),
  );
}
