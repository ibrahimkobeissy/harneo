import * as Sentry from "@sentry/react";
import { mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import {
  bootErrorReporting,
  createQuotaGuard,
  MAX_EVENTS_PER_RUN,
  containsTokenShape,
  scrubBreadcrumb,
  scrubEvent,
  scrubText,
  startErrorReporting,
} from "./errorReporting";

const FAKE_DSN = "https://public@example.invalid/1";

function mockTransport() {
  const sent: unknown[] = [];
  const transport = () => ({
    send: (envelope: unknown) => {
      sent.push(envelope);
      return Promise.resolve({});
    },
    flush: () => Promise.resolve(true),
  });
  return { sent, transport };
}

afterEach(async () => {
  await Sentry.close();
});

describe("scrubbing", () => {
  it("turns the home folder into ~", () => {
    expect(scrubText("/home/ana/x")).toBe("~/x");
    expect(scrubText("at /Users/bo/y z")).toBe("at ~/y z");
    expect(scrubText("C:\\Users\\cy\\z")).toBe("~\\z");
  });

  it("truncates messages to 500 characters", () => {
    expect(Array.from(scrubText("é".repeat(900)))).toHaveLength(500);
  });

  it("drops breadcrumb data", () => {
    const crumb = scrubBreadcrumb({
      message: "open /home/ana/a",
      data: { body: "secret" },
    });
    expect(crumb.data).toBeUndefined();
    expect(crumb.message).toBe("open ~/a");
  });

  it("scrubs an event everywhere and drops request, extra and file contents", () => {
    const event = scrubEvent({
      type: undefined,
      message: "failed /home/ana/x",
      extra: { file: "contents" },
      request: { url: "http://x", data: "body" },
      user: { id: "1" },
      breadcrumbs: [{ message: "m", data: { a: 1 } }],
      exception: {
        values: [
          {
            value: "bad /home/ana/q",
            stacktrace: {
              frames: [
                {
                  abs_path: "/home/ana/src/a.ts",
                  context_line: "const x = 1;",
                  pre_context: ["a"],
                },
              ],
            },
          },
        ],
      },
    });
    expect(event?.message).toBe("failed ~/x");
    expect(event?.extra).toBeUndefined();
    expect(event?.request).toBeUndefined();
    expect(event?.user).toBeUndefined();
    expect(event?.breadcrumbs?.[0]?.data).toBeUndefined();
    const ex = event?.exception?.values?.[0];
    expect(ex?.value).toBe("bad ~/q");
    const frame = ex?.stacktrace?.frames?.[0];
    expect(frame?.abs_path).toBe("~/src/a.ts");
    expect(frame?.context_line).toBeUndefined();
    expect(frame?.pre_context).toBeUndefined();
  });

  it("drops an event containing a token shape", () => {
    const token = ["gh", "p_", "a1B2".repeat(10)].join("");
    expect(containsTokenShape(token)).toBe(true);
    expect(
      containsTokenShape(["-----BEGIN RSA ", "PRIVATE KEY-----"].join("")),
    ).toBe(true);
    expect(containsTokenShape("plain /home/me/file.ts")).toBe(false);
    expect(scrubEvent({ type: undefined, message: `x ${token}` })).toBeNull();
  });
});

describe("starting", () => {
  it("creates no client with reporting off or no DSN", () => {
    expect(startErrorReporting({ enabled: false, dsn: null }, "1.0.0")).toBe(
      false,
    );
    expect(startErrorReporting({ enabled: true, dsn: null }, "1.0.0")).toBe(
      false,
    );
    expect(startErrorReporting(undefined, "1.0.0")).toBe(false);
    expect(Sentry.getClient()).toBeUndefined();
  });

  it("boots from the backend and stays off when it says off", async () => {
    mockIPC((cmd) => {
      if (cmd === "error_reporting_config")
        return { enabled: false, dsn: null };
      return undefined;
    });
    expect(await bootErrorReporting()).toBe(false);
    expect(Sentry.getClient()).toBeUndefined();
  });

  it("boots from the backend and creates a client when it says on", async () => {
    mockIPC((cmd) => {
      if (cmd === "error_reporting_config")
        return { enabled: true, dsn: FAKE_DSN };
      if (cmd === "app_info") return { version: "1.2.3", dataDir: "/d" };
      return undefined;
    });
    expect(await bootErrorReporting()).toBe(true);
    expect(Sentry.getClient()?.getOptions().release).toBe("harneo@1.2.3");
  });

  it("sends a tagged, scrubbed error through a mock transport", async () => {
    const { sent, transport } = mockTransport();
    expect(
      startErrorReporting({ enabled: true, dsn: FAKE_DSN }, "1.2.3", transport),
    ).toBe(true);
    Sentry.captureException(new Error("failed /home/ana/x"));
    await Sentry.flush(2000);
    expect(sent).toHaveLength(1);
    const text = JSON.stringify(sent[0]);
    expect(text).toContain('"side":"webview"');
    expect(text).toContain("harneo@1.2.3");
    expect(text).toContain("failed ~/x");
    expect(text).not.toContain("/home/ana");
    expect(Sentry.getClient()?.getOptions().tracesSampleRate).toBe(0);
  });
});

describe("quota guard", () => {
  it("sends an identical error once and caps the run at 20 events", () => {
    const allow = createQuotaGuard();
    const msg = (message: string) => ({ type: undefined, message });
    expect(allow(msg("same"))).toBe(true);
    expect(allow(msg("same"))).toBe(false);
    for (let i = 1; i < MAX_EVENTS_PER_RUN; i++)
      expect(allow(msg(`e${i}`))).toBe(true);
    expect(allow(msg("one more"))).toBe(false);
  });
});

describe("scrubbing details", () => {
  it("scrubs log entries, module names and frame paths, and drops params", () => {
    const event = scrubEvent({
      type: undefined,
      logentry: { message: "x /home/ana/y", params: ["secret"] },
      exception: {
        values: [
          {
            module: "/home/ana/m",
            stacktrace: {
              frames: [{ filename: "/home/ana/f.ts", module: "/home/ana/g" }],
            },
          },
        ],
      },
    });
    expect(event?.logentry?.message).toBe("x ~/y");
    expect(event?.logentry?.params).toBeUndefined();
    expect(event?.exception?.values?.[0]?.module).toBe("~/m");
    const frame = event?.exception?.values?.[0]?.stacktrace?.frames?.[0];
    expect(frame?.filename).toBe("~/f.ts");
    expect(frame?.module).toBe("~/g");
  });

  it("drops an event it cannot inspect", () => {
    const loop: Record<string, unknown> = {};
    loop.self = loop;
    expect(
      scrubEvent({ type: undefined, extra: loop } as Sentry.ErrorEvent),
    ).toBeNull();
  });

  it("dedupes by exception type and message", () => {
    const allow = createQuotaGuard();
    const ex = (value: string) => ({
      type: undefined,
      exception: { values: [{ type: "Error", value }] },
    });
    expect(allow(ex("a"))).toBe(true);
    expect(allow(ex("a"))).toBe(false);
    expect(allow(ex("b"))).toBe(true);
  });

  it("stays off when the backend call fails", async () => {
    mockIPC(() => {
      throw new Error("no backend");
    });
    expect(await bootErrorReporting()).toBe(false);
  });
});
