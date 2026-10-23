import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Facets } from "../../bindings/Facets";
import { parseParams } from "./params";
import { ResultsToolbar } from "./ResultsToolbar";

const facets: Facets = {
  tiers: [],
  kinds: [],
  tags: [],
  sources: [
    {
      source: { id: "s1", kind: "git", label: "acme/pack", branch: "main" },
      count: 3,
    },
    {
      source: { id: "s2", kind: "local", label: "my-skills", branch: null },
      count: 1,
    },
  ],
};

function setup(query: string) {
  const onRemove = vi.fn();
  render(
    <ResultsToolbar
      params={parseParams(new URLSearchParams(query))}
      facets={facets}
      total={2}
      unfilteredTotal={9}
      filtered
      onRemove={onRemove}
      onClearText={vi.fn()}
      onClearAll={vi.fn()}
    />,
  );
  return onRemove;
}

describe("ResultsToolbar", () => {
  it("labels source chips with their branch, or the bare label, and removes them", async () => {
    const onRemove = setup("source=s1&source=s2&tag=rust");
    expect(screen.getByText("2 of 9 artifacts")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Remove filter acme/pack @ main" }),
    );
    expect(onRemove).toHaveBeenCalledWith("sources", "s1");
    await userEvent.click(
      screen.getByRole("button", { name: "Remove filter my-skills" }),
    );
    expect(onRemove).toHaveBeenCalledWith("sources", "s2");
    await userEvent.click(
      screen.getByRole("button", { name: "Remove filter rust" }),
    );
    expect(onRemove).toHaveBeenCalledWith("tags", "rust");
  });

  it("falls back to the source id when the source is not in the facets", () => {
    setup("source=gone");
    expect(
      screen.getByRole("button", { name: "Remove filter gone" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Save as smart collection/ }),
    ).toBeNull();
  });
});
