import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it } from "vitest";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Library: live rescan", () => {
  it("reloads the list when the library changes on disk, not for a project", async () => {
    let loads = 0;
    mockIPC(
      (cmd) => {
        if (cmd !== "library_list") return undefined;
        loads += 1;
        return page(
          loads === 1 ? [item("alpha")] : [item("alpha"), item("beta")],
          {
            total: loads === 1 ? 1 : 2,
          },
        );
      },
      { shouldMockEvents: true },
    );
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await screen.findByRole("button", { name: /alpha/ });
    await emit("harneo://changed", { scope: "project", id: 1 });
    await new Promise((r) => setTimeout(r, 30));
    expect(loads).toBe(1);
    await emit("harneo://changed", { scope: "library" });
    expect(await screen.findByRole("button", { name: /beta/ })).toBeVisible();
    await waitFor(() => expect(loads).toBe(2));
  });
});
