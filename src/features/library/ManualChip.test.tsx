import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { item } from "../../test/library";
import { ItemCard } from "./ItemCard";
import { ItemTable } from "./ItemTable";

describe("manual-only marker", () => {
  it("shows on a card and a list row only when the skill is manual only", () => {
    const { rerender } = render(
      <ItemCard item={item("a", { manualOnly: true })} onOpen={() => {}} />,
    );
    expect(screen.getByText("Manual only")).toBeInTheDocument();
    rerender(<ItemCard item={item("a")} onOpen={() => {}} />);
    expect(screen.queryByText("Manual only")).toBeNull();
  });

  it("shows in the list", () => {
    render(
      <ItemTable
        items={[item("a", { manualOnly: true }), item("b")]}
        onOpen={() => {}}
        sort="name"
        descending={false}
        onSort={() => {}}
      />,
    );
    expect(screen.getAllByText("Manual only")).toHaveLength(1);
  });
});
