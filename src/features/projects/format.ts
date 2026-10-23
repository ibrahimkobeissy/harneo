import type { Project } from "../../bindings/Project";

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

const UNITS: [number, string][] = [
  [86400, "day"],
  [3600, "hour"],
  [60, "minute"],
];

/** "just now", "5 minutes ago", "3 days ago"; a date after two weeks. */
export function ago(seconds: bigint, now = Date.now()): string {
  const diff = Math.max(0, Math.floor(now / 1000) - Number(seconds));
  if (diff >= 14 * 86400) return dateOf(seconds);
  for (const [size, name] of UNITS) {
    if (diff >= size) return `${plural(Math.floor(diff / size), name)} ago`;
  }
  return "just now";
}

export function dateOf(seconds: bigint): string {
  return new Date(Number(seconds) * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function dateTimeOf(seconds: bigint): string {
  return new Date(Number(seconds) * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export type Status = { kind: "sync" } | { kind: "available"; version: number };
// The Drift ticket (DR3) adds `{ kind: "drift"; files: number }` here and its pill in `StatusPill`.

/** In sync, unless the profile has moved on since the project last applied it. */
export function statusOf(
  project: Project,
  currentVersion: number | undefined,
): Status {
  if (currentVersion !== undefined && currentVersion > project.profileVersion) {
    return { kind: "available", version: currentVersion };
  }
  return { kind: "sync" };
}

/** UTF-8 text as the base64 a `CodeView` takes. */
export function toBase64(text: string): string {
  let raw = "";
  for (const byte of new TextEncoder().encode(text)) {
    raw += String.fromCharCode(byte);
  }
  return btoa(raw);
}

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
