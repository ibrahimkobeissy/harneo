import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { ChangeScope } from "../bindings/ChangeScope";
import { useChanged } from "./useChanged";

afterEach(() => {
  cleanup();
  clearMocks();
});

function Probe({ onScope }: { onScope: (scope: ChangeScope) => void }) {
  useChanged(onScope);
  return null;
}

describe("useChanged", () => {
  it("calls the newest callback on each harneo://changed event", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const first: ChangeScope[] = [];
    const second: ChangeScope[] = [];
    const { rerender } = render(<Probe onScope={(s) => first.push(s)} />);
    await new Promise((r) => setTimeout(r, 20));
    await emit("harneo://changed", { scope: "library" });
    rerender(<Probe onScope={(s) => second.push(s)} />);
    await emit("harneo://changed", { scope: "project", id: 1 });
    expect(first).toEqual([{ scope: "library" }]);
    expect(second).toEqual([{ scope: "project", id: 1 }]);
  });

  it("stops listening on unmount, even before the listener is ready", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const seen: ChangeScope[] = [];
    render(<Probe onScope={(s) => seen.push(s)} />).unmount();
    const live = render(<Probe onScope={(s) => seen.push(s)} />);
    await new Promise((r) => setTimeout(r, 20));
    live.unmount();
    await emit("harneo://changed", { scope: "library" });
    expect(seen).toEqual([]);
  });

  it("does nothing when the event bridge is missing", async () => {
    // No mockIPC: listening fails, which the hook swallows.
    const seen: ChangeScope[] = [];
    render(<Probe onScope={(s) => seen.push(s)} />);
    await new Promise((r) => setTimeout(r, 20));
    expect(seen).toEqual([]);
  });
});
