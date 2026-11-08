import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { LibraryItem } from "../../bindings/LibraryItem";
import { item, page } from "../../test/library";
import { Picker } from "./Picker";

interface Q {
  filter: { text: string | null };
  offset: number;
  limit: number;
}

function stub(all: LibraryItem[]) {
  const queries: Q[] = [];
  mockIPC((cmd, args) => {
    if (cmd !== "library_list") throw new Error(`unexpected ${cmd}`);
    const q = (args as { query: Q }).query;
    queries.push(q);
    const hits = all.filter(
      (i) => !q.filter.text || i.name.includes(q.filter.text),
    );
    return page(hits.slice(q.offset, q.offset + q.limit), {
      total: hits.length,
    });
  });
  return queries;
}

function renderPicker(picked: string[] = []) {
  const onAdd = vi.fn();
  const onRemove = vi.fn();
  const onAddAll = vi.fn();
  const onRemoveAll = vi.fn();
  render(
    <Picker
      picked={new Set(picked)}
      onAdd={onAdd}
      onRemove={onRemove}
      onAddAll={onAddAll}
      onRemoveAll={onRemoveAll}
    />,
  );
  return { onAdd, onRemove, onAddAll, onRemoveAll };
}

describe("Picker", () => {
  it("shows trust on its own line: amber with findings, plain when clean, not audited", async () => {
    stub([
      item("risky", { trustScore: 41, findingCount: 3, bundleSize: 1 }),
      item("one", { trustScore: 80, findingCount: 1 }),
      item("clean", { trustScore: 90 }),
      item("raw"),
    ]);
    renderPicker();
    const risky = (await screen.findByText("risky")).closest(
      "li",
    ) as HTMLElement;
    expect(within(risky).getByText("bundle of 2 · agent-skills")).toBeVisible();
    expect(within(risky).getByText("trust 41 · 3 findings").className).toMatch(
      /warn/,
    );
    expect(screen.getByText("trust 80 · 1 finding").className).toMatch(/warn/);
    expect(screen.getByText("trust 90").className).not.toMatch(/warn/);
    expect(screen.getByText("not audited").className).not.toMatch(/warn/);
  });

  it("counts rows, pages with Show more and resets on a new search", async () => {
    const user = userEvent.setup();
    const all = Array.from({ length: 95 }, (_, i) => item(`a${i}`));
    const queries = stub(all);
    renderPicker();
    expect(await screen.findByText("Showing 40 of 95")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Show more (40)" }));
    expect(await screen.findByText("Showing 80 of 95")).toBeVisible();
    expect(queries.at(-1)?.limit).toBe(80);
    await user.click(screen.getByRole("button", { name: "Show more (15)" }));
    expect(await screen.findByText("Showing 95 of 95")).toBeVisible();
    expect(screen.queryByRole("button", { name: /Show more/ })).toBeNull();
    await user.type(
      screen.getByRole("textbox", { name: "Search library" }),
      "a9",
    );
    await waitFor(() => expect(queries.at(-1)?.limit).toBe(40));
    expect(await screen.findByText("Showing 6 of 6")).toBeVisible();
  });

  it("filters to Added and Not added", async () => {
    const user = userEvent.setup();
    stub([item("one"), item("two")]);
    renderPicker(["id-one"]);
    await screen.findByText("two");
    const show = screen.getByRole("group", { name: "Show" });
    await user.click(within(show).getByRole("button", { name: "Added" }));
    expect(screen.getByText("one")).toBeVisible();
    expect(screen.queryByText("two")).toBeNull();
    expect(screen.getByText("1 of the 2 loaded shown (2 match)")).toBeVisible();
    await user.click(within(show).getByRole("button", { name: "Not added" }));
    expect(screen.queryByText("one")).toBeNull();
    expect(screen.getByText("two")).toBeVisible();
  });

  it("says when the view has nothing and when the search has no matches", async () => {
    const user = userEvent.setup();
    stub([item("one")]);
    renderPicker();
    await screen.findByText("one");
    const show = screen.getByRole("group", { name: "Show" });
    await user.click(within(show).getByRole("button", { name: "Added" }));
    expect(screen.getByText("Nothing here yet.")).toBeVisible();
    await user.type(
      screen.getByRole("textbox", { name: "Search library" }),
      "zzz",
    );
    expect(await screen.findByText("No matches.")).toBeVisible();
  });

  it("names the buttons with their visible label and exposes pressed state", async () => {
    const user = userEvent.setup();
    stub([item("one"), item("two")]);
    const { onAdd, onRemove } = renderPicker(["id-one"]);
    const added = await screen.findByRole("button", {
      name: "Added one, remove",
    });
    expect(added).toHaveAttribute("aria-pressed", "true");
    expect(added).toHaveTextContent("Added");
    const add = screen.getByRole("button", { name: "Add two" });
    expect(add).toHaveAttribute("aria-pressed", "false");
    await user.click(add);
    await user.click(added);
    expect(onAdd).toHaveBeenCalledWith(
      expect.objectContaining({ name: "two" }),
    );
    expect(onRemove).toHaveBeenCalledWith("id-one");
  });

  it("shows a load failure", async () => {
    mockIPC(() => {
      throw new Error("down");
    });
    renderPicker();
    expect(await screen.findByRole("alert")).toHaveTextContent("did not load");
  });

  it("adds or removes every artifact shown, after the search, in one click", async () => {
    const user = userEvent.setup();
    stub([item("alpha"), item("alps"), item("beta")]);
    const { onAddAll, onRemoveAll } = renderPicker([item("alps").id]);
    await screen.findByText("Showing 3 of 3");
    expect(
      screen.getByRole("button", { name: "Add all shown (2)" }),
    ).toBeEnabled();
    await user.type(
      screen.getByRole("textbox", { name: "Search library" }),
      "al",
    );
    await screen.findByText("Showing 2 of 2");
    await user.click(screen.getByRole("button", { name: "Add all shown (1)" }));
    expect(onAddAll).toHaveBeenCalledWith([
      expect.objectContaining({ name: "alpha" }),
    ]);
    await user.click(
      screen.getByRole("button", { name: "Remove all shown (1)" }),
    );
    expect(onRemoveAll).toHaveBeenCalledWith([item("alps").id]);
  });

  it("disables a bulk button that would do nothing", async () => {
    stub([item("alpha")]);
    renderPicker([item("alpha").id]);
    await screen.findByText("Showing 1 of 1");
    expect(
      screen.getByRole("button", { name: "Add all shown (0)" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Remove all shown (1)" }),
    ).toBeEnabled();
  });
});
