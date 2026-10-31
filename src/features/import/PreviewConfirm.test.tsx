import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Audit } from "../../bindings/Audit";
import type { Candidate } from "../../bindings/Candidate";
import type { Finding } from "../../bindings/Finding";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import { ImportScreen } from ".";

/** What phase 1 flags in session-radar's JSX: a critical that is not a threat. */
const flagged: Finding = {
  rule: "secret-in-text",
  severity: "critical",
  message: "Contains a secret in plain text",
  file: "skills/session-radar/SKILL.md",
  line: 9,
  evidence: "<Button key=…>",
};
const reach = { files: [], network: [], shell: [], env: [], protects: 0 };

function audit(score: number, tier: Audit["tier"], findings: Finding[]): Audit {
  return {
    score,
    tier,
    findings,
    reach,
    dismissed: [],
    confirmation: null,
  };
}

function cand(name: string, a: Audit): Candidate {
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
    audit: a,
  } as Candidate;
}

const ticket = {
  token: "tok",
  source: { kind: "local", path: "/work/shop" },
  diagnostics: [],
  unplaced: [],
  unplacedOverflow: 0,
  layoutIgnored: [],
  candidates: [
    cand("session-radar", audit(10, "dangerous", [flagged, flagged])),
    cand("wild", audit(70, "review", [{ ...flagged, severity: "medium" }])),
    cand("calm", audit(100, "trusted", [])),
  ],
} as PreviewTicket;

/** What Claude answers for session-radar: both findings are JSX, not a leak. */
const dismissed: Audit = {
  score: 100,
  tier: "trusted",
  findings: [],
  reach,
  dismissed: [flagged, flagged].map((finding) => ({
    finding,
    reason: "It is JSX, not a credential.",
  })),
  confirmation: {
    status: "confirmed",
    model: "haiku",
    at: null,
    detail: null,
    kind: null,
  },
};

function setup(opts: { on?: boolean; confirmError?: string } = {}) {
  const calls: string[] = [];
  // Phase 2 runs until `finish` ends them, as a real run ends after its last candidate.
  const running: (() => void)[] = [];
  mockIPC(
    (cmd) => {
      calls.push(cmd);
      switch (cmd) {
        case "home_claude_dir":
          return "/home/me/.claude";
        case "library_preview":
          return ticket;
        case "tags_list":
          return [];
        case "audit_confirm_setting_get":
          return opts.on ?? true;
        case "claude_cli_status":
          return { found: true, path: "/bin/claude", version: "2" };
        case "library_preview_confirm":
          if (opts.confirmError) throw new Error(opts.confirmError);
          return new Promise<void>((done) => running.push(done));
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  const view = render(
    <MemoryRouter initialEntries={["/import"]}>
      <Routes>
        <Route path="/import" element={<ImportScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  const finish = () =>
    act(() => {
      for (const done of running.splice(0)) done();
    });
  return { calls, view, finish };
}

async function toChoose(u: ReturnType<typeof userEvent.setup>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found 3 artifacts/);
}

function verdict(id: string, a: Audit, done: number) {
  return act(() =>
    emit("preview-confirm-progress", {
      token: "tok",
      done,
      total: 2,
      candidate: { id, audit: a },
    }),
  );
}

const failedAudit: Audit = {
  ...ticket.candidates[0]!.audit,
  confirmation: {
    status: "failed",
    model: "haiku",
    at: null,
    detail: "Claude's usage limit is reached. Try again later.",
    kind: "limit",
  },
};

const started = (calls: string[]) =>
  waitFor(() => expect(calls).toContain("library_preview_confirm"));

describe("Import: phase 2 on the Choose step", () => {
  it("starts in the background and lifts the automatic exclusion when the verdicts clear the findings", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    expect(screen.getByText("2 to import, 1 excluded")).toBeVisible();
    await act(() =>
      emit("preview-confirm-progress", {
        token: "tok",
        done: 0,
        total: 2,
        candidate: null,
      }),
    );
    expect(
      await screen.findByText("Confirming 1 of 2 with Claude…"),
    ).toBeVisible();
    // The choices wait for Claude: the list is inert and Continue is off.
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    expect(
      screen.getByText("2 to import, 1 excluded").closest("[inert]"),
    ).not.toBeNull();

    await verdict("id-session-radar", dismissed, 1);
    expect(await screen.findByText("3 to import, 0 excluded")).toBeVisible();
    const skills = screen.getByRole("region", { name: /^Skills/ });
    expect(within(skills).getAllByText("100").length).toBeGreaterThan(1);
    const note = within(skills).getByRole("button", {
      name: /Claude: not a threat \(2\)/,
    });
    await u.click(note);
    expect(
      within(skills).getAllByText(/It is JSX, not a credential\./),
    ).toHaveLength(2);
    expect(screen.getByText("Confirming 2 of 2 with Claude…")).toBeVisible();
  });

  it("never changes a candidate the user excluded by hand", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    // Restored by hand, then excluded again by hand: the automatic rule no longer owns it.
    await u.click(
      screen.getByRole("button", { name: "Restore session-radar" }),
    );
    await u.click(screen.getByRole("button", { name: "Restore anyway" }));
    await u.click(
      screen.getByRole("button", { name: "Exclude session-radar" }),
    );
    await u.click(screen.getByRole("button", { name: "Exclude wild" }));
    expect(screen.getByText("1 to import, 2 excluded")).toBeVisible();
    await verdict("id-session-radar", dismissed, 1);
    await verdict("id-wild", { ...dismissed, dismissed: [] }, 2);
    expect(screen.getByRole("region", { name: /^Excluded/ })).toBeVisible();
    expect(screen.getByText("1 to import, 2 excluded")).toBeVisible();
  });

  it("lets the user continue before phase 2 ends; Review follows the verdicts", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByRole("button", { name: /Import 2 artifacts/ }),
    ).toBeEnabled();
    expect(
      screen.getByRole("region", { name: "Security audit" }),
    ).toHaveTextContent("1 artifact held back for critical findings");
    await verdict("id-session-radar", dismissed, 1);
    expect(
      await screen.findByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeEnabled();
    expect(
      screen.getByRole("region", { name: "Security audit" }),
    ).not.toHaveTextContent("held back");
  });

  it("stops the run when the import is left", async () => {
    const u = userEvent.setup();
    const { calls, view } = setup();
    await toChoose(u);
    await started(calls);
    view.unmount();
    await waitFor(() =>
      expect(calls).toContain("library_preview_confirm_cancel"),
    );
  });

  it("makes no call and shows the Phase 1 only warning when phase 2 is off", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ on: false });
    await toChoose(u);
    expect(await screen.findByText(/Phase 1 only\./)).toBeVisible();
    expect(
      screen.getByRole("link", { name: /Turn on Confirm security findings/ }),
    ).toHaveAttribute("href", "/settings#claude-settings");
    expect(calls).not.toContain("library_preview_confirm");
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByText(/Phase 1 only\./)).toBeVisible();
    expect(calls).not.toContain("library_preview_confirm");
  });

  it("pauses on a failure with three choices and imports nothing until one is picked", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    await verdict("id-session-radar", failedAudit, 1);
    expect(
      await screen.findByText(
        /Claude could not confirm the findings: Claude's usage limit is reached\. Try again later\. Findings are shown as the rules found them\./,
      ),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Cancel import" })).toBeEnabled();
    await u.click(
      screen.getByRole("button", { name: "Continue without Claude" }),
    );
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    // Findings stay as the rules found them, marked as unconfirmed.
    expect(
      screen.getByText(/Findings are shown as the rules found them\./),
    ).toBeVisible();
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByRole("button", { name: /Import \d artifacts?/ }),
    ).toBeEnabled();
  });

  it("holds Import on the Review step while a failure waits for a choice", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByRole("button", { name: /Import 2 artifacts/ });
    await verdict("id-session-radar", failedAudit, 1);
    expect(
      await screen.findByRole("button", { name: /Import \d artifacts?/ }),
    ).toBeDisabled();
    expect(screen.getByRole("button", { name: "Retry" })).toBeVisible();
  });

  it("Cancel import leaves for the library", async () => {
    const u = userEvent.setup();
    const { calls } = setup();
    await toChoose(u);
    await started(calls);
    await verdict("id-session-radar", failedAudit, 1);
    await u.click(await screen.findByRole("button", { name: "Cancel import" }));
    await waitFor(() =>
      expect(calls).toContain("library_preview_confirm_cancel"),
    );
    expect(screen.queryByText(/Found 3 artifacts/)).toBeNull();
  });

  it("Retry runs phase 2 again on the held preview and clears the pause when it succeeds", async () => {
    const u = userEvent.setup();
    const { calls, finish } = setup();
    await toChoose(u);
    await started(calls);
    await verdict("id-session-radar", failedAudit, 1);
    await u.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() =>
      expect(calls.filter((c) => c === "library_preview_confirm")).toHaveLength(
        2,
      ),
    );
    expect(
      await screen.findByRole("button", { name: "Retrying…" }),
    ).toBeDisabled();
    await verdict("id-session-radar", dismissed, 1);
    await waitFor(() =>
      expect(screen.queryByText(/Claude could not confirm/)).toBeNull(),
    );
    // The retried run is still going: the choices wait for it to end.
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await finish();
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
    expect(screen.queryByText(/with Claude…/)).toBeNull();
  });

  it("shows why the run itself failed instead of swallowing it", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ confirmError: "the library is busy" });
    await toChoose(u);
    await started(calls);
    expect(
      await screen.findByText(
        /Claude could not confirm the findings: the library is busy\./,
      ),
    ).toBeVisible();
  });
});
