import type { Confirmation } from "../bindings/Confirmation";

/** True when phase 2 ran into a problem: Claude was missing, or the run failed. */
export function confirmationFailed(
  c: Confirmation | null | undefined,
): boolean {
  return c?.status === "failed" || c?.status === "unavailable";
}

/** Why Claude could not confirm, as one plain sentence without its final period. */
export function confirmationReason(c: Confirmation): string {
  const detail = c.detail?.trim();
  const text =
    detail ||
    (c.status === "unavailable"
      ? "the Claude CLI is not available"
      : "no reason was given");
  return text.replace(/\.+$/, "");
}
