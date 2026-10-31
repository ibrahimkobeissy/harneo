import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { Detection } from "../../bindings/Detection";
import type { Preview } from "../../bindings/Preview";
import { ImportScreen } from ".";

function candidate(name: string): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind: "skill",
      name,
      description: `About ${name}`,
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
    suggestedTags: name === "lead" ? ["review"] : [],
    copies: [],
    audit: {
      score: 100,
      tier: "trusted",
      findings: [],
      reach: { files: [], network: [], shell: [], env: [], protects: 0 },
      unaudited: [],
      dismissed: [],
      confirmation: null,
    },
  };
}

const preview: Preview = {
  source: { kind: "local", path: "/home/me/.claude" },
  diagnostics: [],
  unplaced: [],
  unplacedOverflow: 0,
  layoutIgnored: [],
  candidates: ["lead", "helper", "other"].map(candidate),
};

const detection: Detection = {
  links: [
    {
      from: "id-lead",
      to: "id-helper",
      reason: "Hands off to the helper.",
      confidence: "high",
    },
    {
      from: "id-lead",
      to: "id-other",
      reason: "Hands off to the helper.",
      confidence: "high",
    },
    {
      from: "id-helper",
      to: "lib-1",
      reason: "Calls the library skill.",
      confidence: "medium",
    },
  ],
  tags: [
    { artifact: "id-lead", tags: ["Review", "testing"] },
    { artifact: "id-helper", tags: ["docs"] },
  ],
};

interface Call {
  cmd: string;
  args: unknown;
}

function setup(opts: {
  cli?: { found: boolean; path: string | null; version: string | null };
  atImport?: boolean | null;
  detect?: () => Detection | Promise<Detection>;
}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview":
        return preview;
      case "library_import":
        return {
          sourceId: "s",
          imported: [],
          alreadyInLibrary: [],
          skippedFiles: [],
        };
      case "tags_list":
        return [];
      case "settings_get": {
        const { key } = args as { key: string };
        return key === "bundles.atImport"
          ? opts.atImport === undefined
            ? true
            : opts.atImport
          : null;
      }
      case "claude_cli_status":
        return opts.cli ?? { found: true, path: "/bin/claude", version: "2.0" };
      case "bundles_detect_preview":
        return opts.detect ? opts.detect() : detection;
      case "library_detail":
        return { artifact: { name: "library-skill" } };
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
  return calls;
}

async function toReview(u: ReturnType<typeof userEvent.setup>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found 3 artifacts/);
  await u.click(screen.getByRole("button", { name: "Continue" }));
}

describe("Review step detection", () => {
  it("shows progress while Claude reads, then the proposed bundles grouped by lead", async () => {
    let release: (d: Detection) => void = () => {};
    setup({ detect: () => new Promise<Detection>((r) => (release = r)) });
    const u = userEvent.setup();
    await toReview(u);
    expect(
      await screen.findByText("Claude is reading 3 artifacts…"),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeDisabled();
    release(detection);
    expect(await screen.findByText("needs 2")).toBeVisible();
    expect(screen.getAllByText("Hands off to the helper.")).toHaveLength(1);
    expect(await screen.findByText("library-skill")).toBeVisible();
    expect(
      screen.getByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeEnabled();
  });

  it("merges Claude's tags with the keyword ones once, and tags stay removable", async () => {
    setup({});
    const u = userEvent.setup();
    await toReview(u);
    await screen.findByText("needs 2");
    const add = screen.getByRole("button", { name: "Add a tag to lead" });
    const row = add.closest("[role=group]");
    if (!(row instanceof HTMLElement)) throw new Error("row missing");
    expect(within(row).getAllByText(/^(review|testing)$/i)).toHaveLength(2);
    await u.click(
      within(row).getByRole("button", { name: "Remove testing from lead" }),
    );
    expect(within(row).queryByText("testing")).toBeNull();
  });

  it("imports with the accepted groups only, minus dropped members", async () => {
    const calls = setup({});
    const u = userEvent.setup();
    await toReview(u);
    await screen.findByText("needs 2");
    await u.click(
      screen.getByRole("button", {
        name: "Remove other from the bundle of lead",
      }),
    );
    await u.click(
      screen.getByRole("switch", { name: "Accept bundle of helper" }),
    );
    await u.click(screen.getByRole("button", { name: /Import 3 artifacts/ }));
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "library_import")).toBe(true),
    );
    const imp = calls.find((c) => c.cmd === "library_import");
    expect((imp?.args as { links: unknown }).links).toEqual([
      ["id-lead", "id-helper", "Hands off to the helper."],
    ]);
    // The refused group leads from a preview candidate, so nothing is remembered.
    expect(calls.filter((c) => c.cmd === "link_reject")).toHaveLength(0);
  });

  it("says so when there are no bundles", async () => {
    setup({ detect: () => ({ links: [], tags: [] }) });
    const u = userEvent.setup();
    await toReview(u);
    expect(
      await screen.findByText(/these artifacts don't depend on each other/),
    ).toBeVisible();
  });

  it("explains a missing CLI and keeps importing possible", async () => {
    const calls = setup({ cli: { found: false, path: null, version: null } });
    const u = userEvent.setup();
    await toReview(u);
    expect(await screen.findByText("Claude CLI not found.")).toBeVisible();
    expect(screen.getByText(/npm install -g/)).toBeVisible();
    expect(screen.getByRole("link", { name: "Settings" })).toBeVisible();
    expect(screen.getByText(/Library → Find bundles/)).toBeVisible();
    // Without Claude the tags come from keywords only.
    expect(screen.getByText(/from each artifact's keywords:/)).toBeVisible();
    expect(calls.some((c) => c.cmd === "bundles_detect_preview")).toBe(false);
    expect(
      screen.getByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeEnabled();
  });

  it("shows the failure and retries", async () => {
    let attempts = 0;
    setup({
      detect: () => {
        attempts += 1;
        if (attempts === 1) throw new Error("timed out");
        return detection;
      },
    });
    const u = userEvent.setup();
    await toReview(u);
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Claude couldn't finish reading.");
    expect(alert).toHaveTextContent("Claude's message: timed out");
    expect(within(alert).getByRole("link", { name: "Settings" })).toBeVisible();
    await u.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("needs 2")).toBeVisible();
  });

  it("does not run when it is off in Settings, until Detect now", async () => {
    const calls = setup({ atImport: false });
    const u = userEvent.setup();
    await toReview(u);
    expect(
      await screen.findByText(/Bundle and tag suggestions are off/),
    ).toBeVisible();
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute(
      "href",
      "/settings",
    );
    expect(calls.some((c) => c.cmd === "bundles_detect_preview")).toBe(false);
    await u.click(
      screen.getByRole("button", { name: "Detect now for this import" }),
    );
    expect(await screen.findByText("needs 2")).toBeVisible();
  });

  it("sends nothing to Claude on a fresh install, where the setting was never set", async () => {
    const calls = setup({ atImport: null });
    const u = userEvent.setup();
    await toReview(u);
    expect(
      await screen.findByText(/Bundle and tag suggestions are off/),
    ).toBeVisible();
    expect(calls.some((c) => c.cmd === "bundles_detect_preview")).toBe(false);
    expect(calls.some((c) => c.cmd === "claude_cli_status")).toBe(false);
  });

  it("leads with 'needs N', says what accepting does and counts links on the Import button", async () => {
    setup({});
    const u = userEvent.setup();
    await toReview(u);
    await screen.findByText("needs 2");
    expect(
      screen.getByText(/Switch off to stop Claude proposing this again/),
    ).toBeVisible();
    expect(screen.getAllByText("Accepted")).toHaveLength(2);
    // The helper group has medium confidence.
    expect(screen.getByText("medium confidence")).toBeVisible();
    expect(screen.queryByText("high confidence")).toBeNull();
    expect(
      screen.getByRole("button", { name: "Import 3 artifacts and 3 links" }),
    ).toBeEnabled();
    expect(screen.getByText(/keywords and Claude:/)).toBeVisible();
    await u.click(
      screen.getByRole("switch", { name: "Accept bundle of helper" }),
    );
    expect(screen.getByText("Skipped")).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Import 3 artifacts and 2 links" }),
    ).toBeEnabled();
  });

  it("singularises one link", async () => {
    setup({ detect: () => ({ links: detection.links.slice(2), tags: [] }) });
    const u = userEvent.setup();
    await toReview(u);
    await screen.findByText("needs 1");
    expect(
      screen.getByRole("button", { name: "Import 3 artifacts and 1 link" }),
    ).toBeVisible();
  });

  it("shows an indeterminate bar until the first batch, and can skip detection while it runs", async () => {
    const calls = setup({
      detect: () => new Promise<Detection>(() => undefined),
    });
    const u = userEvent.setup();
    await toReview(u);
    const bar = await screen.findByRole("progressbar", {
      name: "Detection progress",
    });
    expect(bar).not.toHaveAttribute("value");
    expect(
      screen.getByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeDisabled();
    await u.click(screen.getByRole("button", { name: "Skip detection" }));
    expect(screen.queryByRole("button", { name: "Skip detection" })).toBeNull();
    expect(screen.getByText(/Detection was skipped/)).toBeVisible();
    expect(
      screen.getByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeEnabled();
    // Detect now picks it up again.
    await u.click(screen.getByRole("button", { name: "Detect now" }));
    expect(
      await screen.findByRole("button", { name: "Skip detection" }),
    ).toBeVisible();
    const runs = calls.filter((c) => c.cmd === "bundles_detect_preview");
    expect(runs).toHaveLength(2);
    // The automatic run relies on the setting; Detect now is the user's own request.
    expect(
      runs.map((r) => (r.args as { userInitiated: boolean }).userInitiated),
    ).toEqual([false, true]);
  });
});
