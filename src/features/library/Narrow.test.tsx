import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useNavigate } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const facets = {
  tiers: [],
  withFindings: 0,
  outdated: 0,
  kinds: [{ kind: "skill" as const, count: 2 }],
  tags: [{ name: "review", color: "violet" as const, count: 1 }],
  sources: [],
};

function narrow(matches: boolean) {
  vi.stubGlobal(
    "matchMedia",
    vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    })),
  );
}

function GoOpen() {
  const navigate = useNavigate();
  return (
    <button
      type="button"
      onClick={() => void navigate("/library?open=id-alpha")}
    >
      go
    </button>
  );
}

function setup(path = "/library") {
  mockIPC((cmd) =>
    cmd === "library_list"
      ? page([item("alpha")], { total: 2, facets })
      : undefined,
  );
  render(
    <MemoryRouter initialEntries={[path]}>
      <GoOpen />
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => narrow(true));
afterEach(() => vi.unstubAllGlobals());

describe("half-width library", () => {
  it("hides the column behind a Filters button that counts active filters", async () => {
    setup("/library?type=skill&tag=review");
    const button = await screen.findByRole("button", { name: /Filters/ });
    expect(button).toHaveTextContent("2");
    expect(screen.queryByRole("complementary")).not.toBeInTheDocument();
    // The chips stay visible in the toolbar.
    expect(
      screen.getByRole("button", { name: "Remove filter Skills" }),
    ).toBeInTheDocument();
  });

  it("opens the same column as a sheet, traps focus and closes on Esc back to the button", async () => {
    const user = userEvent.setup();
    setup();
    const button = await screen.findByRole("button", { name: /Filters/ });
    await user.click(button);
    const sheet = screen.getByRole("dialog", { name: "Filters" });
    expect(
      within(sheet).getByRole("button", { name: /Skills/ }),
    ).toBeInTheDocument();
    expect(sheet).toContainElement(document.activeElement as HTMLElement);
    for (let i = 0; i < 12; i++) await user.tab();
    expect(sheet).toContainElement(document.activeElement as HTMLElement);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(button).toHaveFocus();
  });

  it("closes the sheet when a link opens the detail drawer", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(await screen.findByRole("button", { name: /Filters/ }));
    expect(screen.getByRole("dialog", { name: "Filters" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "go" }));
    expect(
      await screen.findByRole("dialog", { name: /details/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("dialog", { name: "Filters" }),
    ).not.toBeInTheDocument();
  });

  it("closes from the scrim and the close button", async () => {
    const user = userEvent.setup();
    setup();
    const button = await screen.findByRole("button", { name: /Filters/ });
    await user.click(button);
    await user.click(screen.getByRole("button", { name: "Close filters" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(button);
    const scrim = document.querySelector<HTMLElement>(
      '[aria-hidden="true"][class*="sheetScrim"]',
    );
    expect(scrim).not.toBeNull();
    await user.click(scrim as HTMLElement);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(button).toHaveFocus();
  });

  it("keeps the inline column and no button at full width", async () => {
    narrow(false);
    setup();
    expect(await screen.findByRole("complementary")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^Filters/ }),
    ).not.toBeInTheDocument();
  });
});
