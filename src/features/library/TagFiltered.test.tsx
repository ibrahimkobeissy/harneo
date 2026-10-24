import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const existing = { id: 1, name: "old", color: "cyan", count: 3 };

function setup(path: string) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === "library_list") {
      return page([item("skill-1"), item("skill-2")], {
        total: 37,
        unfilteredTotal: 90,
      });
    }
    if (cmd === "tags_list") return [existing];
    if (cmd === "tag_filtered") return existing;
    return undefined;
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  const tagged = () => calls.filter((c) => c.cmd === "tag_filtered");
  return { calls, tagged };
}

describe("Tag… on the filtered artifacts", () => {
  it("is offered only while a filter is active", async () => {
    setup("/library");
    await screen.findByText("skill-1");
    expect(screen.queryByRole("button", { name: /Tag…/ })).toBeNull();
  });

  it("applies an existing tag to the whole filter, not the visible page", async () => {
    const { tagged, calls } = setup("/library?tag=old");
    await userEvent.click(await screen.findByRole("button", { name: /Tag…/ }));
    await userEvent.click(await screen.findByRole("option", { name: /old/ }));
    await waitFor(() => expect(tagged()).toHaveLength(1));
    const args = tagged()[0]?.args as {
      filter: { tags: string[] };
      name: string;
      color: string | null;
    };
    expect(args.filter.tags).toEqual(["old"]);
    expect(args.name).toBe("old");
    expect(args.color).toBeNull();
    // The list reloads so the facets and chips show the new tag.
    await waitFor(() =>
      expect(calls.filter((c) => c.cmd === "library_list").length).toBe(2),
    );
  });

  it("creates a new tag with the chosen color", async () => {
    const { tagged } = setup("/library?tag=old");
    await userEvent.click(await screen.findByRole("button", { name: /Tag…/ }));
    await userEvent.type(await screen.findByRole("combobox"), "brand new");
    await userEvent.click(screen.getByRole("radio", { name: "red" }));
    await userEvent.click(
      await screen.findByRole("option", { name: /Create “brand new”/ }),
    );
    await waitFor(() => expect(tagged()).toHaveLength(1));
    expect(tagged()[0]?.args).toMatchObject({
      name: "brand new",
      color: "red",
    });
  });
});
