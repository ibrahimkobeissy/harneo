import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppRoutes } from "../App";
import { item, page } from "../test/library";

function renderApp() {
  return render(
    <MemoryRouter initialEntries={["/library"]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  window.localStorage.clear();
  mockIPC((cmd) => (cmd === "library_list" ? page([item("one")]) : undefined));
});
afterEach(() => vi.restoreAllMocks());

describe("collapsible sidebar", () => {
  it("hides to the left, comes back, and is remembered", async () => {
    const user = userEvent.setup();
    const first = renderApp();
    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));
    expect(window.localStorage.getItem("harneo.sidebar.collapsed")).toBe(
      "true",
    );
    // The sidebar is inert and invisible; only the edge button remains.
    expect(
      screen.getByRole("navigation", { name: "Main" }).parentElement,
    ).toHaveAttribute("inert");
    first.unmount();

    renderApp();
    await user.click(screen.getByRole("button", { name: "Show sidebar" }));
    expect(
      screen.getByRole("navigation", { name: "Main" }).parentElement,
    ).not.toHaveAttribute("inert");
    expect(window.localStorage.getItem("harneo.sidebar.collapsed")).toBe(
      "false",
    );
  });

  it("toggles with Ctrl+B and lists the shortcut", async () => {
    const user = userEvent.setup();
    renderApp();
    await user.keyboard("{Control>}b{/Control}");
    expect(
      screen.getByRole("button", { name: "Show sidebar" }),
    ).toBeInTheDocument();
    await user.keyboard("{Control>}b{/Control}");
    expect(
      screen.getByRole("button", { name: "Hide sidebar" }),
    ).toBeInTheDocument();
    await user.keyboard("?");
    expect(screen.getByText("Show or hide the sidebar")).toBeInTheDocument();
  });

  it("still works when storage throws", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const user = userEvent.setup();
    renderApp();
    await user.click(screen.getByRole("button", { name: "Hide sidebar" }));
    expect(
      screen.getByRole("button", { name: "Show sidebar" }),
    ).toBeInTheDocument();
  });
});
