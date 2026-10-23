import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import type { AuditSeverity } from "../../bindings/AuditSeverity";
import type { Reach } from "../../bindings/Reach";
import type { Tier } from "../../bindings/Tier";
import { SecurityScreen } from ".";

const none: Reach = { files: [], network: [], shell: [], env: [] };

function entry(
  name: string,
  score: number | null,
  over: {
    findings?: [string, AuditSeverity, string][];
    reach?: Partial<Reach>;
    kind?: ArtifactAudit["kind"];
  } = {},
): ArtifactAudit {
  const tier: Tier =
    score === null
      ? "trusted"
      : score >= 80
        ? "trusted"
        : score >= 60
          ? "review"
          : score >= 40
            ? "risky"
            : "dangerous";
  return {
    id: `id-${name}`,
    kind: over.kind ?? "skill",
    name,
    audit:
      score === null
        ? null
        : {
            score,
            tier,
            findings: (over.findings ?? []).map(
              ([rule, severity, evidence]) => ({
                rule,
                severity,
                message: `${rule} message`,
                file: `skills/${name}/SKILL.md`,
                line: 4,
                evidence,
              }),
            ),
            reach: { ...none, ...over.reach },
          },
  };
}

const sample: ArtifactAudit[] = [
  entry("alpha", 100),
  entry("bravo", 85, {
    findings: [["hidden-unicode", "low", "zero width"]],
    reach: { network: ["api.example.com"] },
  }),
  entry("charlie", 55, {
    findings: [["rce", "critical", "curl | sh"]],
    reach: { shell: ["curl"], network: ["evil.test"], env: ["TOKEN"] },
    kind: "mcp",
  }),
  entry("delta", 30, {
    findings: [["plaintext-secrets", "high", "token=***"]],
    reach: { files: ["~/.config"] },
  }),
];

function Where() {
  const loc = useLocation();
  return <p data-testid="where">{loc.pathname + loc.search}</p>;
}

function setup(
  audits: ArtifactAudit[],
  reaudit = { audited: audits.length, changed: 0 },
) {
  const calls: string[] = [];
  mockIPC((cmd) => {
    calls.push(cmd);
    if (cmd === "library_audits") return audits;
    if (cmd === "library_reaudit") return reaudit;
    return null;
  });
  render(
    <MemoryRouter initialEntries={["/security"]}>
      <Routes>
        <Route path="/security" element={<SecurityScreen />} />
        <Route path="/library" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

function names(table: HTMLElement): string[] {
  return within(table)
    .getAllByRole("link")
    .map((a) => a.textContent ?? "");
}

describe("Security page", () => {
  it("counts tiers, severities and reach", async () => {
    setup(sample);
    const trust = await screen.findByRole("region", { name: "Trust scores" });
    expect(within(trust).getByText("4 artifacts")).toBeInTheDocument();
    expect(within(trust).getByRole("img")).toHaveAccessibleName(
      "Trust score distribution: 2 trusted, 0 review, 1 risky, 1 dangerous",
    );
    expect(within(trust).getByText("80–100")).toBeInTheDocument();
    const findings = screen.getByRole("region", { name: "Findings" });
    expect(within(findings).getByText("3 in 3 artifacts")).toBeInTheDocument();
    const reach = screen.getByRole("region", {
      name: "What artifacts can reach",
    });
    expect(
      within(reach).getByRole("button", { name: /2\s+network hosts/ }),
    ).toBeInTheDocument();
  });

  it("sorts the findings table with aria-sort, severity first", async () => {
    setup(sample);
    const table = await screen.findByRole("table", { name: "Findings" });
    expect(names(table)).toEqual(["charlie", "delta", "bravo"]);
    const sev = within(table).getByRole("columnheader", { name: "Severity" });
    expect(sev).toHaveAttribute("aria-sort", "descending");
    expect(
      within(table).getByRole("columnheader", { name: "Artifact" }),
    ).toHaveAttribute("aria-sort", "none");

    await userEvent.click(
      within(table).getByRole("button", { name: "Artifact" }),
    );
    expect(names(table)).toEqual(["bravo", "charlie", "delta"]);
    expect(
      within(table).getByRole("columnheader", { name: "Artifact" }),
    ).toHaveAttribute("aria-sort", "ascending");
    await userEvent.click(
      within(table).getByRole("button", { name: "Artifact" }),
    );
    expect(names(table)).toEqual(["delta", "charlie", "bravo"]);

    await userEvent.click(within(table).getByRole("button", { name: "Trust" }));
    expect(names(table)).toEqual(["delta", "charlie", "bravo"]);
    await userEvent.click(within(table).getByRole("button", { name: "Trust" }));
    expect(names(table)).toEqual(["bravo", "charlie", "delta"]);
  });

  it("lists only artifacts that reach beyond their files, sortable", async () => {
    setup(sample);
    const table = await screen.findByRole("table", { name: "Reach" });
    expect(names(table)).toEqual(["delta", "charlie", "bravo"]);
    expect(
      within(table).getByRole("columnheader", { name: "Trust" }),
    ).toHaveAttribute("aria-sort", "ascending");
    await userEvent.click(
      within(table).getByRole("button", { name: "Network" }),
    );
    expect(names(table)).toEqual(["delta", "bravo", "charlie"]);
    await userEvent.click(
      within(table).getByRole("button", { name: "Network" }),
    );
    expect(names(table)[2]).toBe("delta");
  });

  it("filters the reach table by a tile and clears on a second click", async () => {
    setup(sample);
    const table = await screen.findByRole("table", { name: "Reach" });
    const tile = screen.getByRole("button", { name: /shell/ });
    await userEvent.click(tile);
    expect(tile).toHaveAttribute("aria-pressed", "true");
    expect(names(table)).toEqual(["charlie"]);
    expect(
      screen.getByText("filtered: run shell commands"),
    ).toBeInTheDocument();
    await userEvent.click(tile);
    expect(tile).toHaveAttribute("aria-pressed", "false");
    expect(names(table)).toHaveLength(3);

    await userEvent.click(screen.getByRole("button", { name: /env vars/ }));
    await userEvent.click(screen.getByRole("button", { name: "Show all" }));
    expect(names(table)).toHaveLength(3);
  });

  it("opens an artifact in the library detail panel", async () => {
    setup(sample);
    const table = await screen.findByRole("table", { name: "Findings" });
    await userEvent.click(within(table).getByRole("link", { name: "charlie" }));
    expect(await screen.findByTestId("where")).toHaveTextContent(
      "/library?open=id-charlie",
    );
  });

  it("re-runs the audit, shows the result and reloads", async () => {
    const calls = setup(sample, { audited: 4, changed: 2 });
    await screen.findByRole("table", { name: "Findings" });
    await userEvent.click(screen.getByRole("button", { name: "Re-run audit" }));
    expect(
      await screen.findByText("Re-audited 4 artifacts: 2 changed"),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(calls.filter((c) => c === "library_audits")).toHaveLength(2),
    );
    expect(calls).toContain("library_reaudit");
  });

  it("says so when the library is empty", async () => {
    setup([]);
    expect(
      await screen.findByRole("heading", { name: "Nothing to audit yet" }),
    ).toBeInTheDocument();
  });

  it("suggests Re-run audit when nothing is audited yet", async () => {
    const calls = setup([entry("old", null)], { audited: 1, changed: 1 });
    expect(
      await screen.findByRole("heading", { name: "Nothing audited yet" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/Choose Re-run audit/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Re-run audit" }));
    await waitFor(() => expect(calls).toContain("library_reaudit"));
  });

  it("shows the no-findings state and an empty reach table", async () => {
    setup([entry("alpha", 100), entry("beta", 95)]);
    expect(
      await screen.findByRole("heading", { name: "No findings" }),
    ).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Reach" });
    expect(within(table).getByText("None.")).toBeInTheDocument();
  });

  it("reports a failed read", async () => {
    mockIPC(() => {
      throw "database locked";
    });
    render(
      <MemoryRouter>
        <SecurityScreen />
      </MemoryRouter>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "database locked",
    );
  });
});
