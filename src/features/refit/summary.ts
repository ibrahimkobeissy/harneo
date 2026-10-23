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

export function lossUnit(loss: Loss): string {
  const one = loss.count === 1;
  return loss.unit === "permissionRules"
    ? one
      ? "permission rule"
      : "permission rules"
    : one
      ? "line"
      : "lines";
}

export function lossText(loss: Loss): string {
  return `${loss.count} ${lossUnit(loss)} will be lost`;
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

/** A critical row that would lose content with its chosen action. */
export interface CriticalLoss {
  row: Row;
  loss: Loss;
}

export function criticalLosses(
  rows: readonly Row[],
  choices: Record<string, RefitAction>,
): CriticalLoss[] {
  const out: CriticalLoss[] = [];
  for (const row of rows) {
    const loss = isCritical(row.item) ? lossOf(row, choices) : null;
    if (loss) out.push({ row, loss });
  }
  return out;
}

/** "17 lines", "3 permission rules" or "17 lines and rules" for the losses taken together. */
export function totalLoss(losses: readonly CriticalLoss[]): string {
  const total = losses.reduce((n, l) => n + l.loss.count, 0);
  const rules = losses.filter((l) => l.loss.unit === "permissionRules").length;
  if (rules === 0) return `${total} ${total === 1 ? "line" : "lines"}`;
  if (rules === losses.length)
    return `${total} permission ${total === 1 ? "rule" : "rules"}`;
  return `${total} lines and rules`;
}

/** The same path spelled differently ("a/b/", "./a//b", "a\\b") compares equal. */
export function samePath(a: string, b: string): boolean {
  const norm = (p: string) => {
    const parts = p
      .trim()
      .replace(/\\/g, "/")
      .split("/")
      .filter((s) => s !== "" && s !== ".");
    return (p.trim().startsWith("/") ? "/" : "") + parts.join("/");
  };
  return norm(a) === norm(b);
}
