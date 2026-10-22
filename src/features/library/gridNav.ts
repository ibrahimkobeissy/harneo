import type { KeyboardEvent } from "react";

/** How many columns the grid lays out, read from the browser's resolved track list. */
function columnCount(grid: HTMLElement): number {
  const tracks = getComputedStyle(grid).gridTemplateColumns.trim();
  return tracks === "" || tracks === "none" ? 1 : tracks.split(/\s+/).length;
}

/**
 * Arrow keys, Home and End move focus between the cards of a grid. Left and Right step one
 * card, Up and Down one row (as many cards as there are columns).
 */
export function onGridKeyDown(event: KeyboardEvent<HTMLElement>) {
  const grid = event.currentTarget;
  const cards = Array.from(
    grid.querySelectorAll<HTMLElement>("[data-item-id]"),
  );
  const at = cards.findIndex((card) => card === event.target);
  if (at < 0 || event.ctrlKey || event.metaKey || event.altKey) return;
  const columns = columnCount(grid);
  const moves: Record<string, number> = {
    ArrowLeft: at - 1,
    ArrowRight: at + 1,
    ArrowUp: at - columns,
    ArrowDown: at + columns,
    Home: 0,
    End: cards.length - 1,
  };
  const to = moves[event.key];
  if (to === undefined) return;
  event.preventDefault();
  cards[Math.min(Math.max(to, 0), cards.length - 1)]?.focus();
}
