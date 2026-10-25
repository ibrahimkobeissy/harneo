import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactAudit } from "../../bindings/ArtifactAudit";
import type { Audit } from "../../bindings/Audit";
import type { Confirmation } from "../../bindings/Confirmation";
import { ArtifactPanelHost } from "../../app/ArtifactPanelHost";
import { ArtifactPanelProvider } from "../../lib/artifactPanel";
import { minimalDetail } from "../../test/artifactDetail";
import { SecurityScreen } from ".";

const finding = {
  rule: "prompt-injection",
  severity: "critical" as const,
  message: "Tells the model to ignore its instructions",
  file: "skills/alpha/SKILL.md",
  line: 4,
  evidence: "ignore previous instructions",
};

function audit(confirmation: Confirmation | null, dismissed = false): Audit {
  return {
    score: 55,
    tier: "risky",
    findings: [{ ...finding, rule: "rce" }],
    reach: { files: [], network: [], shell: [], env: [], protects: 0 },
    dismissed: dismissed
      ? [{ finding, reason: "It warns the reader against the attack." }]
      : [],
    confirmation,
  };
}

const status = (s: Confirmation["status"]): Confirmation => ({
  status: s,
  model: "haiku",
  at: null,
  detail: null,
  kind: null,
});

function setup(opts: {
  on?: boolean;
  cli?: boolean;
  confirmation?: Confirmation | null;
  dismissed?: boolean;
}) {
  const state = { confirmation: opts.confirmation ?? status("confirmed") };
  let releaseAll: () => void = () => undefined;
  mockIPC(
    (cmd, args) => {
      if (cmd === "tags_list" || cmd === "artifact_tags") return [];
      if (cmd === "library_detail") {
        return {
          ...minimalDetail((args as { id: string }).id),
          audit: audit(state.confirmation, opts.dismissed),
        };
      }
      if (cmd === "audit_confirm_setting_get") return opts.on ?? true;
      if (cmd === "claude_cli_status")
        return opts.cli === false
          ? { found: false, path: null, version: null }
          : { found: true, path: "/bin/claude", version: "2" };
      if (cmd === "library_audits") {
        const a: ArtifactAudit = {
          id: "id-alpha",
          kind: "skill",
          name: "alpha",
          audit: audit(state.confirmation, opts.dismissed),
        };
        return [a];
      }
      if (cmd === "audit_confirm_all")
        return new Promise<void>((resolve) => {
          releaseAll = () => {
            state.confirmation = status("confirmed");
            resolve();
          };
        });
      return null;
    },
    { shouldMockEvents: true },
  );
  render(
    <MemoryRouter initialEntries={["/security"]}>
      <ArtifactPanelProvider>
        <Routes>
          <Route path="/security" element={<SecurityScreen />} />
        </Routes>
        <ArtifactPanelHost />
      </ArtifactPanelProvider>
    </MemoryRouter>,
  );
  return { release: () => releaseAll() };
}

const warning = /Phase 1 only\./;

describe("Security: phase 2", () => {
  it("shows no warning when phase 2 is on and working", async () => {
    setup({});
    await screen.findByText("Findings by artifact");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Confirm now" })).toBeEnabled(),
    );
    expect(screen.queryByText(warning)).not.toBeInTheDocument();
  });

  it("warns, linking to the setting, when phase 2 is off", async () => {
    setup({ on: false });
    expect(await screen.findByText(warning)).toBeVisible();
    expect(
      screen.getByText(/include false positives; Claude has not confirmed/),
    ).toBeVisible();
    expect(
      screen.getByRole("link", { name: /Turn on Confirm security findings/ }),
    ).toHaveAttribute("href", "/settings#claude-settings");
    expect(screen.getByRole("button", { name: "Confirm now" })).toBeDisabled();
  });

  it("warns and points to the CLI setup when the CLI is missing", async () => {
    setup({ cli: false });
    expect(await screen.findByText(warning)).toBeVisible();
    expect(
      screen.getByRole("link", { name: /Set up the Claude CLI/ }),
    ).toBeVisible();
  });

  it("shows progress, marks pending artifacts and refreshes when Confirm now is done", async () => {
    const user = userEvent.setup();
    const { release } = setup({ confirmation: status("pending") });
    await screen.findByText("pending");
    const button = screen.getByRole("button", { name: "Confirm now" });
    await waitFor(() => expect(button).toBeEnabled());
    await user.click(button);
    await act(() => emit("audit-confirm-progress", { done: 1, total: 3 }));
    expect(
      await screen.findByText("Confirming 2 of 3 with Claude…"),
    ).toBeVisible();
    await act(async () => release());
    await waitFor(() =>
      expect(screen.queryByText(/Confirming/)).not.toBeInTheDocument(),
    );
    expect(screen.queryByText("pending")).not.toBeInTheDocument();
    await user.click(
      await screen.findByRole("button", { name: /Show findings of alpha/ }),
    );
    expect(screen.getByText("Confirmed by Claude")).toBeVisible();
  });

  it("lists dismissed findings with Claude's reason in the detail panel, not in the counts", async () => {
    const user = userEvent.setup();
    setup({ dismissed: true });
    await user.click(
      (await screen.findAllByRole("button", { name: "alpha" }))[0]!,
    );
    const summary = await screen.findByText("Dismissed by Claude (1)");
    await user.click(summary);
    expect(
      screen.getByText(/It warns the reader against the attack\./),
    ).toBeVisible();
    expect(screen.getByText("1 in 1 artifact")).toBeVisible();
  });
});
