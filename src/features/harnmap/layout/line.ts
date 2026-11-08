import { assemble, rowOf, type Layout, type LayoutInput } from "./common";

/** Stages left to right on a 144 px pitch, stations stacked under each, loops as arcs below. */
export function layoutLine(input: LayoutInput): Layout {
  return assemble(
    input,
    (_, widths) => rowOf(widths).map((x) => ({ x, y: 0 })),
    () => ({}),
  );
}
