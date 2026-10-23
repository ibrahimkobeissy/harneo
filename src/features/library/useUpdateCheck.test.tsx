import { mockIPC } from "@tauri-apps/api/mocks";
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CHECKED_EVENT } from "../../lib/updates";
import { useUpdateCheck } from "./useUpdateCheck";

describe("useUpdateCheck", () => {
  it("reports sources that are gone, ignores current ones, and notifies", () => {
    mockIPC(() => null);
    let notified = 0;
    const { result } = renderHook(() => useUpdateCheck(() => notified++));
    act(() => {
      window.dispatchEvent(
        new CustomEvent(CHECKED_EVENT, {
          detail: {
            sources: [
              { id: "s1", result: { type: "missing" } },
              { id: "s2", result: { type: "unchanged" } },
            ],
          },
        }),
      );
    });
    expect(result.current.problems).toEqual([
      { id: "s1", reason: "The source is no longer there." },
    ]);
    expect(notified).toBe(1);
    act(() => result.current.dismiss());
    expect(result.current.problems).toEqual([]);
  });

  it("keeps the failure when the check itself throws", async () => {
    mockIPC((cmd) => {
      if (cmd === "updates_check") throw "no network";
      return null;
    });
    const { result } = renderHook(() => useUpdateCheck(() => {}));
    await act(() => result.current.check());
    await waitFor(() => expect(result.current.failure).toContain("no network"));
    expect(result.current.checking).toBe(false);
  });
});
