import type { Loss } from "../../bindings/Loss";
import type { RefitAction } from "../../bindings/RefitAction";
import type { Row } from "../../bindings/Row";

/** Rows whose loss is shown red: the two files where a replace costs the user their own setup. */
const critical = new Set(["CLAUDE.md", ".claude/settings.json"]);

export function isCritical(item: string): boolean {
  return critical.has(item);
}

export function actionOf(
  row: Row,
  choices: Record<string, RefitAction>,
): RefitAction {
  return choices[row.item] ?? row.defaultAction;
}

/** The loss a row would cause with its chosen action, or null when it loses nothing. */
export function lossOf(
  row: Row,
  choices: Record<string, RefitAction>,
): Loss | null {
  const action = actionOf(row, choices);
  return action === "replace" || action === "delete" ? row.loss : null;
}

export function lossText(loss: Loss): string {
  const one = loss.count === 1;
  const noun =
    loss.unit === "permissionRules"
      ? one
        ? "permission rule"
        : "permission rules"
      : one
        ? "line"
        : "lines";
  return `${loss.count} ${noun} will be lost`;
}

export function baseName(path: string): string {
  const parts = path.trim().split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function counts(
  rows: readonly Row[],
  choices: Record<string, RefitAction>,
): Record<RefitAction, number> {
  const total: Record<RefitAction, number> = {
    keep: 0,
    replace: 0,
    delete: 0,
    add: 0,
    skip: 0,
  };
  for (const row of rows) total[actionOf(row, choices)] += 1;
  return total;
}
