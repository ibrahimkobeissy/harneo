import * as Sentry from "@sentry/react";
import type { ReportingConfig } from "../bindings/ReportingConfig";
import { appInfo, errorReportingConfig } from "./ipc";

/** Messages are cut to this many characters (the Rust side uses the same limit). */
export const MAX_MESSAGE = 500;

/**
 * Token shapes, mirrored from `SHAPES` in `crates/harneo-core/src/audit/rules/leaks.rs` (the
 * audit's leaks rule, which the Rust scrubber reaches through `contains_token_shape`). Keep the
 * two lists in step; the test here pins one sample per family.
 */
const TOKEN_SHAPES: RegExp[] = [
  /\bgh[pousr]_[A-Za-z0-9]{30,}/,
  /\bgithub_pat_[A-Za-z0-9_]{22,}/,
  /\bxox[abposr]-[A-Za-z0-9-]{10,}/,
  /\bsk-ant-[A-Za-z0-9_-]{20,}/,
  /\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}/,
  /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/,
  /\bAIza[A-Za-z0-9_-]{35}/,
  /\b[sr]k_live_[A-Za-z0-9]{16,}/,
  /\beyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}/,
  /-----BEGIN [A-Z ]*PRIVATE KEY-----/,
  /\bbearer\s+[A-Za-z0-9._~+/=-]{20,}/i,
];

/** Whether the text holds a known token shape. */
export function containsTokenShape(text: string): boolean {
  return TOKEN_SHAPES.some((re) => re.test(text));
}

const USER_DIR =
  /(?:\/home|\/Users)\/[^/\\\s"']+|[A-Za-z]:\\Users\\[^\\\s"']+/g;

/** Replaces the home folder with `~` and cuts the text to `MAX_MESSAGE` characters. */
export function scrubText(text: string): string {
  const swapped = text.replace(USER_DIR, "~");
  return Array.from(swapped).slice(0, MAX_MESSAGE).join("");
}

function scrubFrame(frame: Sentry.StackFrame): Sentry.StackFrame {
  const next: Sentry.StackFrame = { ...frame };
  if (next.filename) next.filename = scrubText(next.filename);
  if (next.abs_path) next.abs_path = scrubText(next.abs_path);
  if (next.module) next.module = scrubText(next.module);
  // Source lines and variables are file contents: never attached.
  delete next.pre_context;
  delete next.context_line;
  delete next.post_context;
  delete next.vars;
  return next;
}

function scrubTrace(trace: Sentry.Stacktrace | undefined) {
  if (trace?.frames) trace.frames = trace.frames.map(scrubFrame);
}

/** A breadcrumb without its data payload, its text scrubbed. */
export function scrubBreadcrumb(crumb: Sentry.Breadcrumb): Sentry.Breadcrumb {
  const next: Sentry.Breadcrumb = { ...crumb };
  delete next.data;
  if (next.message) next.message = scrubText(next.message);
  return next;
}

function hasToken(event: Sentry.ErrorEvent): boolean {
  try {
    return containsTokenShape(JSON.stringify(event));
  } catch {
    // An event that cannot be inspected is not sent.
    return true;
  }
}

/** The event as it may leave the machine, or `null` when its text holds a token shape. */
export function scrubEvent(event: Sentry.ErrorEvent): Sentry.ErrorEvent | null {
  if (hasToken(event)) return null;
  const next: Sentry.ErrorEvent = { ...event };
  if (next.message) next.message = scrubText(next.message);
  if (next.transaction) next.transaction = scrubText(next.transaction);
  if (next.logentry) {
    next.logentry = {
      ...next.logentry,
      message: scrubText(next.logentry.message ?? ""),
    };
    delete next.logentry.params;
  }
  if (next.exception?.values) {
    next.exception = {
      ...next.exception,
      values: next.exception.values.map((ex) => {
        const copy = { ...ex };
        if (copy.value) copy.value = scrubText(copy.value);
        if (copy.module) copy.module = scrubText(copy.module);
        if (copy.stacktrace) {
          copy.stacktrace = { ...copy.stacktrace };
          scrubTrace(copy.stacktrace);
        }
        return copy;
      }),
    };
  }
  if (next.breadcrumbs)
    next.breadcrumbs = next.breadcrumbs.map(scrubBreadcrumb);
  delete next.request;
  delete next.extra;
  delete next.user;
  delete next.server_name;
  return hasToken(next) ? null : next;
}

/** Most events one run sends (the free Sentry plan has a small monthly quota). */
export const MAX_EVENTS_PER_RUN = 20;

/** At most `MAX_EVENTS_PER_RUN` events per run, and an identical error (same type and message
 *  after scrubbing) once per run. */
export function createQuotaGuard() {
  const seen = new Set<string>();
  return (event: Sentry.ErrorEvent): boolean => {
    if (seen.size >= MAX_EVENTS_PER_RUN) return false;
    const ex = event.exception?.values?.[0];
    const key = ex
      ? `${ex.type ?? ""}:${ex.value ?? ""}`
      : `msg:${event.message ?? ""}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  };
}

/** Client options: errors only, scrubbed, tagged `side: webview` with the release. */
export function buildOptions(
  dsn: string,
  version: string,
  transport?: Sentry.BrowserOptions["transport"],
): Sentry.BrowserOptions {
  const allow = createQuotaGuard();
  return {
    dsn,
    release: `harneo@${version}`,
    environment: import.meta.env.DEV ? "development" : "release",
    tracesSampleRate: 0,
    // Errors only: no release-health sessions. Replay and tracing are not installed.
    integrations: (defaults) =>
      defaults.filter((i) => i.name !== "BrowserSession"),
    initialScope: { tags: { side: "webview" } },
    beforeSend: (event) => {
      const scrubbed = scrubEvent(event);
      return scrubbed && allow(scrubbed) ? scrubbed : null;
    },
    beforeBreadcrumb: scrubBreadcrumb,
    ...(transport ? { transport } : {}),
  };
}

/**
 * Starts reporting when the backend says it is on (a DSN in the environment and the Settings
 * switch). With reporting off, no client is created and no DSN reached the webview.
 */
export function startErrorReporting(
  config: ReportingConfig | undefined | null,
  version: string,
  transport?: Sentry.BrowserOptions["transport"],
): boolean {
  if (!config?.enabled || !config.dsn) return false;
  Sentry.init(buildOptions(config.dsn, version, transport));
  return true;
}

/** Asks the backend whether reporting is on and starts it; never throws, never blocks startup. */
export async function bootErrorReporting(): Promise<boolean> {
  try {
    const config = await errorReportingConfig();
    if (!config?.enabled) return false;
    const info = await appInfo();
    return startErrorReporting(config, info.version);
  } catch {
    return false;
  }
}
