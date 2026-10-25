import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const facets = {
  tiers: [],
  withFindings: 0,
  outdated: 0,
  auto: 0,
  manualOnly: 0,
  statuses: [],
  kinds: [{ kind: "skill" as const, count: 2 }],
  tags: [
    { name: "review", color: "cyan", count: 3 },
    { name: "design", color: "violet", count: 0 },
    { name: "frontend", color: "violet", count: 0 },
  ],
  sources: [],
};

function setup(url = "/library") {
  mockIPC((cmd) =>
    cmd === "library_list"
      ? page([item("alpha")], { total: 2, facets })
      : undefined,
  );
  return render(
    <MemoryRouter initialEntries={[url]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

const column = async () =>
  within(await screen.findByRole("complementary", { name: "Filters" }));
const groups = ["Type", "Trust", "Status", "Invocation", "Tag", "Source"];

beforeEach(() => window.localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("filter groups", () => {
  it("starts with Tag collapsed and the rest expanded; headers toggle", async () => {
    const user = userEvent.setup();
    setup();
    const col = await column();
    for (const g of groups) {
      expect(col.getByRole("button", { name: g })).toHaveAttribute(
        "aria-expanded",
        g === "Tag" ? "false" : "true",
      );
    }
    expect(col.queryByRole("button", { name: /review/ })).toBeNull();
    await user.click(col.getByRole("button", { name: /^Tag/ }));
    expect(col.getByRole("button", { name: /^Tag/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(col.getByRole("button", { name: /review/ })).toBeInTheDocument();
    await user.click(col.getByRole("button", { name: "Type" }));
    expect(col.queryByRole("button", { name: /Skills/ })).toBeNull();
  });

  it("collapses and expands every group and remembers the state", async () => {
    const user = userEvent.setup();
    const first = setup();
    let col = await column();
    await user.click(
      col.getByRole("button", { name: "Collapse all filter groups" }),
    );
    for (const g of groups) {
      expect(col.getByRole("button", { name: g })).toHaveAttribute(
        "aria-expanded",
        "false",
      );
    }
    first.unmount();
    setup();
    col = await column();
    expect(col.getByRole("button", { name: "Type" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    await user.click(
      col.getByRole("button", { name: "Expand all filter groups" }),
    );
    for (const g of groups) {
      expect(col.getByRole("button", { name: g })).toHaveAttribute(
        "aria-expanded",
        "true",
      );
    }
  });

  it("shows the active count on a collapsed group", async () => {
    setup("/library?type=skill");
    const col = await column();
    expect(col.getByRole("button", { name: "Type" })).toBeInTheDocument();
    await userEvent.click(col.getByRole("button", { name: /^Type/ }));
    expect(col.getByLabelText("1 active")).toBeInTheDocument();
  });

  it("hides tags with a zero count unless selected", async () => {
    const user = userEvent.setup();
    setup("/library?tag=design");
    const col = await column();
    await user.click(col.getByRole("button", { name: /^Tag/ }));
    expect(col.getByRole("button", { name: /review/ })).toBeInTheDocument();
    expect(col.getByRole("button", { name: /design/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(col.queryByRole("button", { name: /frontend/ })).toBeNull();
  });
});
