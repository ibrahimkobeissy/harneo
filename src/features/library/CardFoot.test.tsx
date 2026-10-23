import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { LibraryItem } from "../../bindings/LibraryItem";
import { ItemCard } from "./ItemCard";
import { importedAge, ScanBadge } from "./ItemMeta";
import { ItemTable } from "./ItemTable";
import { SessionBars } from "./UsageBars";

function item(over: Partial<LibraryItem> = {}): LibraryItem {
  return {
    id: "a",
    kind: "skill",
    name: "alpha",
    description: "d",
    source: { id: "s", kind: "git", label: "acme/kit", branch: "main" },
    tags: [],
    importedAt: 1_700_000_000n,
    errorCount: 0,
    warningCount: 0,
    fileCount: 1,
    referenceCount: 0,
    resolvedReferences: 0,
    trustScore: 90,
    tier: "trusted",
    findingCount: 0,
    bundleSize: 0,
    update: null,
    usage: { sessions: 3, lastUsed: null },
    status: "current",
    ...over,
  } as LibraryItem;
}

const update = { fromVersion: "1", toVersion: "2", localEdits: false };

function badges(over: Partial<LibraryItem>): string[] {
  render(<ItemCard item={item(over)} onOpen={() => {}} />);
  const foot = document.querySelector("[class*=tileMeta]");
  return Array.from(foot?.querySelectorAll("[class*=badge]") ?? []).map(
    (b) => b.textContent ?? "",
  );
}

describe("card foot: one badge, most urgent first", () => {
  it("an update wins over everything", () => {
    expect(
      badges({ update, bundleSize: 2, errorCount: 1, status: "outdated" }),
    ).toEqual(["Update 2"]);
  });
  it("risky findings come before scan counts", () => {
    expect(badges({ tier: "risky", findingCount: 2, warningCount: 1 })).toEqual(
      ["2 findings"],
    );
  });
  it("the Findings status, then scan counts, then outdated, then the bundle", () => {
    expect(badges({ status: "findings", errorCount: 1 })).toEqual(["Findings"]);
  });
  it("scan errors and warnings share one badge", () => {
    expect(badges({ errorCount: 2, warningCount: 1 })).toEqual([
      "2 scan errors · 1 warning",
    ]);
  });
  it("warnings alone say scan once", () => {
    expect(badges({ warningCount: 3 })).toEqual(["3 scan warnings"]);
  });
  it("outdated without an update record", () => {
    expect(badges({ status: "outdated" })).toEqual(["Outdated"]);
  });
  it("the bundle badge is last and neutral", () => {
    expect(badges({ bundleSize: 3 })).toEqual(["Bundle of 3"]);
  });
  it("no badge when nothing needs saying", () => {
    expect(badges({})).toEqual([]);
  });
  it("does not repeat the version on the source line", () => {
    render(<ItemCard item={item({ update })} onOpen={() => {}} />);
    expect(screen.queryByText(/→/)).not.toBeInTheDocument();
  });
});

describe("list row", () => {
  it("puts the scan count in the name cell and the bundle beside the status", () => {
    render(
      <ItemTable
        items={[item({ warningCount: 2, bundleSize: 2, update })]}
        sort="name"
        descending={false}
        onSort={vi.fn()}
        onOpen={vi.fn()}
      />,
    );
    const row = screen.getAllByRole("row")[1] as HTMLElement;
    const cells = within(row).getAllByRole("cell");
    expect(cells).toHaveLength(8);
    expect(cells[0]).toHaveTextContent("2 scan warnings");
    expect(cells[1]).toHaveTextContent("Skill");
    expect(cells[7]).toHaveTextContent("Update 2");
    expect(cells[7]).toHaveTextContent("Bundle of 2");
  });
});

describe("importedAge", () => {
  const now = new Date(1_700_000_000_000 + 86_400_000 * 0.5);
  const at = (days: number) =>
    importedAge(
      item({ importedAt: BigInt(1_700_000_000 - days * 86_400) }),
      new Date(1_700_000_000_000),
    );
  it("reads short", () => {
    expect(importedAge(item(), now)).toBe("today");
    expect(at(2)).toBe("2d ago");
    expect(at(21)).toBe("3w ago");
    expect(at(120)).toBe("4mo ago");
    expect(at(800)).toBe("2y ago");
  });
});

describe("ScanBadge", () => {
  it("renders nothing without counts", () => {
    const { container } = render(<ScanBadge item={item()} short />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe("SessionBars", () => {
  it("lists every week for screen readers", () => {
    render(<SessionBars weekly={[0, 1, 2, 3, 4, 5, 6, 7]} label="Weekly" />);
    const list = screen.getByRole("list");
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(8);
    expect(rows[0]).toHaveTextContent("7 weeks ago: 0 sessions");
    expect(rows[6]).toHaveTextContent("1 week ago: 6 sessions");
    expect(rows[7]).toHaveTextContent("This week: 7 sessions");
  });
});
