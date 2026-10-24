import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { RemovalItem } from "../../bindings/RemovalItem";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

function removal(name: string, profiles: string[] = []): RemovalItem {
  return {
    id: `id-${name}`,
    kind: "skill",
    name,
    profiles: profiles.map((p, i) => ({
      id: BigInt(i + 1),
      name: p,
      version: 1,
    })),
  };
}

// 37 artifacts match; the visible page only shows two of them.
const matches = Array.from({ length: 37 }, (_, i) =>
  removal(`skill-${i + 1}`, i === 0 ? ["Backend"] : []),
);

function setup(removeFails = false) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "library_list") {
      return page([item("skill-1"), item("skill-2")], {
        total: 37,
        unfilteredTotal: 90,
      });
    }
    if (cmd === "library_removal_plan") return { artifacts: matches };
    if (cmd === "library_remove_many") {
      if (removeFails) throw new Error("disk is full");
      return undefined;
    }
    return undefined;
  });
  render(
    <MemoryRouter initialEntries={["/library?tag=old"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  const removed = () => calls.filter((c) => c.cmd === "library_remove_many");
  return { calls, removed };
}

async function openDialog() {
  await userEvent.click(await screen.findByRole("button", { name: /Delete…/ }));
  return screen.findByRole("dialog", { name: "Delete 37 artifacts" });
}

describe("Delete… on the results toolbar", () => {
  it("is hidden while nothing is filtered", async () => {
    mockIPC((cmd) =>
      cmd === "library_list" ? page([item("alpha")], { total: 1 }) : undefined,
    );
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await screen.findByRole("button", { name: /alpha/ });
    expect(screen.queryByRole("button", { name: /Delete…/ })).toBeNull();
  });

  it("names the filter's total, lists every match and warns about profiles", async () => {
    const { removed } = setup();
    const dialog = await openDialog();
    expect(
      within(dialog).getByRole("heading", { name: "Delete 37 artifacts" }),
    ).toBeInTheDocument();
    const list = within(dialog).getByRole("list", {
      name: "Artifacts to delete",
    });
    expect(within(list).getAllByRole("listitem")).toHaveLength(37);
    expect(within(dialog).getByRole("note")).toHaveTextContent(
      "1 of them is used by a profile (Backend)",
    );
    expect(removed()).toHaveLength(0);
  });

  it("removes exactly the listed ids on confirm and keeps the filter", async () => {
    const { removed, calls } = setup();
    const dialog = await openDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Delete" }),
    );
    await waitFor(() => expect(removed()).toHaveLength(1));
    expect(removed()[0]?.args).toEqual({ ids: matches.map((m) => m.id) });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: /Delete 37/ })).toBeNull(),
    );
    await waitFor(() =>
      expect(
        calls.filter((c) => c.cmd === "library_list").length,
      ).toBeGreaterThan(1),
    );
    expect(
      screen.getByRole("button", { name: "Remove filter old" }),
    ).toBeInTheDocument();
  });

  it("removes nothing on cancel", async () => {
    const { removed } = setup();
    const dialog = await openDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Cancel" }),
    );
    expect(screen.queryByRole("dialog", { name: /Delete 37/ })).toBeNull();
    expect(removed()).toHaveLength(0);
  });

  it("shows the error and keeps the dialog open when the removal fails", async () => {
    setup(true);
    const dialog = await openDialog();
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Delete" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "disk is full",
    );
    expect(
      within(dialog).getByRole("button", { name: "Delete" }),
    ).toBeEnabled();
  });
});
