import type { CheckReport } from "../bindings/CheckReport";
import { settingsGet, settingsSet, updatesCheck } from "./ipc";

const DAY_MS = 24 * 60 * 60 * 1000;

/** Fired on `window` after every check (manual or automatic), with the report as `detail`. */
export const CHECKED_EVENT = "harneo:updates-checked";

/** When the last check ran, from the `updates.lastCheck` setting (an ISO string). */
export async function readLastCheck(): Promise<Date | null> {
  const raw = await settingsGet("updates.lastCheck");
  if (typeof raw !== "string") return null;
  const at = new Date(raw);
  return Number.isNaN(at.getTime()) ? null : at;
}

/** Whether the app checks for updates by itself at start (on unless set to `false`). */
export async function readAutoCheck(): Promise<boolean> {
  return (await settingsGet("updates.autoCheck")) !== false;
}

/** Checks every source, records the time and tells the listening screens. */
export async function runCheck(): Promise<{ report: CheckReport; at: Date }> {
  const report = (await updatesCheck({ type: "all" })) ?? { sources: [] };
  const at = new Date();
  await settingsSet("updates.lastCheck", at.toISOString());
  window.dispatchEvent(new CustomEvent(CHECKED_EVENT, { detail: report }));
  return { report, at };
}

/**
 * The check at app start: runs when `updates.autoCheck` is not false and the last check is
 * older than 24 hours (or never happened). Returns whether it ran.
 */
export async function autoCheckIfDue(now: Date = new Date()): Promise<boolean> {
  if (!(await readAutoCheck())) return false;
  const last = await readLastCheck();
  if (last !== null && now.getTime() - last.getTime() < DAY_MS) return false;
  await runCheck();
  return true;
}
