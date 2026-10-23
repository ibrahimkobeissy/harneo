import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const facets = {
  tiers: [],
  withFindings: 0,
  outdated: 0,
  statuses: [],
  kinds: [{ kind: "skill" as const, count: 2 }],
  tags: [],
  sources: [],
};

function setup() {
  mockIPC((cmd) =>
    cmd === "library_list"
      ? page([item("alpha")], { total: 2, facets })
      : undefined,
  );
  return render(
    <MemoryRouter initialEntries={["/library?type=skill"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

const separator = () => screen.getByRole("separator");

beforeEach(() => {
  window.localStorage.clear();
  // jsdom has no pointer capture.
  const proto = HTMLElement.prototype;
  proto.setPointerCapture = vi.fn();
  proto.releasePointerCapture = vi.fn();
  proto.hasPointerCapture = vi.fn(() => true);
});
afterEach(() => vi.restoreAllMocks());

describe("filter column collapse", () => {
  it("collapses, shows a Filters button with the count, and is remembered", async () => {
    const user = userEvent.setup();
    const first = setup();
    await user.click(
      await screen.findByRole("button", { name: "Hide filters" }),
    );
    expect(window.localStorage.getItem("harneo.filters.collapsed")).toBe(
      "true",
    );
    first.unmount();

    setup();
    const back = await screen.findByRole("button", { name: /^Filters/ });
    expect(back).toHaveTextContent("1");
    await user.click(back);
    expect(
      screen.getByRole("button", { name: "Hide filters" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Filters/ })).toBeNull();
  });

  it("works when storage throws", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const user = userEvent.setup();
    setup();
    await user.click(
      await screen.findByRole("button", { name: "Hide filters" }),
    );
    expect(
      screen.getByRole("button", { name: /^Filters/ }),
    ).toBeInTheDocument();
  });
});

describe("filter column separator", () => {
  it("exposes its value and limits", async () => {
    setup();
    await screen.findByRole("separator");
    expect(separator()).toHaveAttribute("aria-orientation", "vertical");
    expect(separator()).toHaveAttribute("aria-valuenow", "228");
    expect(separator()).toHaveAttribute("aria-valuemin", "180");
    expect(separator()).toHaveAttribute("aria-valuemax", "420");
    expect(separator()).toHaveAttribute("tabindex", "0");
  });

  it("resizes by 16px with the arrows, within the limits, and remembers it", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByRole("separator");
    separator().focus();
    await user.keyboard("{ArrowRight}");
    expect(separator()).toHaveAttribute("aria-valuenow", "244");
    expect(window.localStorage.getItem("harneo.filters.width")).toBe("244");
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(separator()).toHaveAttribute("aria-valuenow", "212");
    await user.keyboard("{End}");
    expect(separator()).toHaveAttribute("aria-valuenow", "420");
    await user.keyboard("{ArrowRight}");
    expect(separator()).toHaveAttribute("aria-valuenow", "420");
    await user.keyboard("{Home}{ArrowLeft}");
    expect(separator()).toHaveAttribute("aria-valuenow", "180");
  });

  it("resets to the default on double-click", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByRole("separator");
    separator().focus();
    await user.keyboard("{End}");
    await user.dblClick(separator());
    expect(separator()).toHaveAttribute("aria-valuenow", "228");
  });

  it("restores a remembered width, clamped", async () => {
    window.localStorage.setItem("harneo.filters.width", "9999");
    setup();
    await screen.findByRole("separator");
    expect(separator()).toHaveAttribute("aria-valuenow", "420");
  });

  it("drags with pointer capture", async () => {
    setup();
    await screen.findByRole("separator");
    fireEvent.pointerDown(separator(), { pointerId: 1, clientX: 228 });
    fireEvent.pointerMove(separator(), { pointerId: 1, clientX: 300 });
    expect(separator()).toHaveAttribute("aria-valuenow", "300");
    fireEvent.pointerMove(separator(), { pointerId: 1, clientX: 900 });
    expect(separator()).toHaveAttribute("aria-valuenow", "420");
    fireEvent.pointerUp(separator(), { pointerId: 1 });
    fireEvent.pointerMove(separator(), { pointerId: 1, clientX: 200 });
    expect(separator()).toHaveAttribute("aria-valuenow", "420");
  });
});
