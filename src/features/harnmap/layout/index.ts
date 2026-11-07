import type { Layout, LayoutInput } from "./common";
import { layoutCycle } from "./cycle";
import { layoutDiamond } from "./diamond";
import { layoutLine } from "./line";
import { layoutParallel } from "./parallel";

export * from "./common";
export { layoutCycle, layoutDiamond, layoutLine, layoutParallel };

/** Lays the map out on the shape of its template. Pure: the same input gives the same output. */
export function layout(input: LayoutInput): Layout {
  switch (input.shape) {
    case "line":
      return layoutLine(input);
    case "cycle":
      return layoutCycle(input);
    case "diamond":
      return layoutDiamond(input);
    case "parallel":
      return layoutParallel(input);
  }
}
