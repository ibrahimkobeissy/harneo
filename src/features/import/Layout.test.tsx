import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import { SourceStep } from "./SourceStep";
import { ImportScreen } from ".";
import { placementOf } from "./placement";

const reach = { files: [], network: [], shell: [], env: [], protects: 0 };

function cand(
  name: string,
  kind: "skill" | "command" | "plugin",
  metadata: Record<string, unknown>,
): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind,
      name,
      description: null,
      path: `${kind}s/${name}`,
      files: [],
      metadata,
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
      score: 100,
      tier: "trusted",
      findings: [],
      reach,
      unaudited: [],
      dismissed: [],
      confirmation: null,
    },
  } as Candidate;
}

const found: Candidate[] = [
  cand("tidy", "skill", { layoutBy: "convention" }),
  cand("mk", "plugin", {
    layoutBy: "manifest",
    layoutManifest: "marketplace.json",
  }),
  cand("solo", "plugin", {
    layoutBy: "manifest",
    layoutManifest: "plugin.json",
  }),
];
const asked: Candidate = cand("review", "command", {
  layoutBy: "claude",
  layoutReason: "It is a prompt under prompts/.",
});

function ticket(over: Partial<PreviewTicket> = {}): PreviewTicket {
  return {
    token: "tok",
    source: { kind: "local", path: "/work/shop" },
    diagnostics: [],
    unplaced: [],
    unplacedOverflow: 0,
    layoutIgnored: [],
    candidates: found,
    ...over,
  } as PreviewTicket;
}

const unplacedTicket = ticket({
  unplaced: ["prompts/review.md", "notes/idea.md"],
  unplacedOverflow: 3,
});
const placedTicket = ticket({
  candidates: [...found, asked],
  unplaced: [],
});

interface Opts {
  first?: PreviewTicket;
  layout?: () => PreviewTicket | Promise<PreviewTicket>;
  auto?: boolean;
  confirm?: boolean;
  url?: string;
}

function setup(opts: Opts = {}) {
  const calls: string[] = [];
  const args: Record<string, unknown> = {};
  mockIPC(
    (cmd, a) => {
      calls.push(cmd);
      args[cmd] = a;
      switch (cmd) {
        case "home_claude_dir":
          return "/home/me/.claude";
        case "library_preview":
          return opts.first ?? ticket();
        case "library_preview_stored":
          return opts.first ?? ticket();
        case "tags_list":
          return [];
        case "audit_confirm_setting_get":
          return opts.confirm ?? false;
        case "claude_cli_status":
          return { found: true };
        case "import_layout_setting_get":
          return opts.auto ?? false;
        case "library_preview_layout":
          return (opts.layout ?? (() => placedTicket))();
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  render(
    <MemoryRouter initialEntries={[opts.url ?? "/import"]}>
      <Routes>
        <Route path="/import" element={<ImportScreen />} />
        <Route path="/library" element={<p>the library</p>} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls, args };
}

async function toChoose(u: ReturnType<typeof userEvent.setup>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found \d+ artifacts?/);
}

describe("placementOf", () => {
  it("words each tier and says nothing for a row the scan did not mark", () => {
    expect(placementOf(found[0] as Candidate)?.label).toBe("recognised folder");
    expect(placementOf(found[1] as Candidate)?.label).toBe(
      "from marketplace.json",
    );
    expect(placementOf(found[2] as Candidate)?.label).toBe("from plugin.json");
    expect(placementOf(asked)).toEqual({
      label: "suggested by Claude",
      reason: "It is a prompt under prompts/.",
    });
    expect(placementOf(cand("x", "skill", {}))).toBeNull();
    expect(
      placementOf(cand("x", "skill", { layoutBy: "claude" }))?.reason,
    ).toBeNull();
  });
});

describe("Import: how each row was placed", () => {
  it("labels the rows and shows Claude's reason on keyboard focus", async () => {
    const u = userEvent.setup();
    setup({ first: placedTicket });
    await toChoose(u);
    expect(screen.getByText("recognised folder")).toBeVisible();
    expect(screen.getByText("from marketplace.json")).toBeVisible();
    expect(screen.getByText("from plugin.json")).toBeVisible();
    const label = screen.getByText("suggested by Claude");
    expect(screen.queryByRole("tooltip")).toBeNull();
    await u.tab();
    while (document.activeElement !== label) {
      await u.tab();
    }
    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "It is a prompt under prompts/.",
    );
    expect(label).toHaveAccessibleDescription("It is a prompt under prompts/.");
    await u.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).toBeNull();
    await u.hover(label);
    expect(await screen.findByRole("tooltip")).toBeVisible();
    await u.unhover(label);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});

describe("Import: the unplaced notice", () => {
  it("is absent when everything was placed", async () => {
    const u = userEvent.setup();
    setup();
    await toChoose(u);
    expect(screen.queryByText(/couldn't place/)).toBeNull();
    expect(
      screen.queryByRole("button", { name: /Ask Claude what's in this repo/ }),
    ).toBeNull();
  });

  it("lists the paths on demand and runs the layout by hand, then labels the new rows", async () => {
    const u = userEvent.setup();
    const { calls, args } = setup({ first: unplacedTicket });
    await toChoose(u);
    expect(screen.getByText("5 files Harneo couldn't place")).toBeVisible();
    expect(screen.queryByText("prompts/review.md")).toBeNull();
    await u.click(screen.getByRole("button", { name: "Show the paths" }));
    expect(screen.getByText("prompts/review.md")).toBeVisible();
    expect(screen.getByText("and 3 more")).toBeVisible();
    // Nothing is sent until the user asks.
    expect(calls).not.toContain("library_preview_layout");
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    await waitFor(() =>
      expect(screen.getByText("suggested by Claude")).toBeVisible(),
    );
    expect(args.library_preview_layout).toEqual({ token: "tok" });
    expect(screen.getByText("review")).toBeVisible();
    expect(screen.queryByText(/couldn't place/)).toBeNull();
  });

  it("sends the rows Claude placed through phase 2 too", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ first: unplacedTicket, confirm: true });
    await toChoose(u);
    await waitFor(() => expect(calls).toContain("library_preview_confirm"));
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    await screen.findByText("suggested by Claude");
    await waitFor(() =>
      expect(calls.filter((c) => c === "library_preview_confirm")).toHaveLength(
        2,
      ),
    );
  });

  it("shows progress while it runs", async () => {
    const u = userEvent.setup();
    let release: (t: PreviewTicket) => void = () => undefined;
    setup({
      first: unplacedTicket,
      layout: () => new Promise<PreviewTicket>((r) => (release = r)),
    });
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    expect(await screen.findByText("Reading the repo…")).toBeVisible();
    await act(() => emit("layout-progress", { token: "tok", stage: "asking" }));
    expect(
      await screen.findByText("Asking Claude what's in this repo…"),
    ).toBeVisible();
    // A run for another preview is not ours.
    await act(() =>
      emit("layout-progress", { token: "other", stage: "checking" }),
    );
    expect(screen.queryByText("Checking Claude's answer…")).toBeNull();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await act(async () => release(placedTicket));
    await waitFor(() =>
      expect(
        screen.queryByText("Asking Claude what's in this repo…"),
      ).toBeNull(),
    );
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("runs by itself once the preview exists when the setting is on", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ first: unplacedTicket, auto: true });
    await toChoose(u);
    await waitFor(() => expect(calls).toContain("library_preview_layout"));
    expect(await screen.findByText("suggested by Claude")).toBeVisible();
  });

  it("does not run by itself with the setting on but nothing unplaced", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ auto: true });
    await toChoose(u);
    await waitFor(() => expect(calls).toContain("import_layout_setting_get"));
    expect(calls).not.toContain("library_preview_layout");
  });
});

describe("Import: a failed layout run", () => {
  function failing() {
    let n = 0;
    return () => {
      n += 1;
      if (n === 1) throw new Error("Claude's usage limit is reached");
      return placedTicket;
    };
  }

  it("pauses with the reason and blocks Continue until a choice is made", async () => {
    const u = userEvent.setup();
    setup({ first: unplacedTicket, layout: failing() });
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    expect(
      await screen.findByText(
        /Claude could not read the repo's layout: Claude's usage limit is reached\./,
      ),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
    await u.click(
      screen.getByRole("button", { name: "Continue with what was found" }),
    );
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    // The note stays, and so does the import with tiers 1 and 2 only.
    expect(screen.getByText(/Only what the manifests/)).toBeVisible();
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByRole("button", { name: /Import 3 artifacts/ }),
    ).toBeEnabled();
  });

  it("Retry asks again and clears the pause on success", async () => {
    const u = userEvent.setup();
    const { calls } = setup({ first: unplacedTicket, layout: failing() });
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    await u.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() =>
      expect(calls.filter((c) => c === "library_preview_layout")).toHaveLength(
        2,
      ),
    );
    expect(await screen.findByText("suggested by Claude")).toBeVisible();
    expect(screen.queryByText(/Claude could not read/)).toBeNull();
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("Cancel import leaves for the library", async () => {
    const u = userEvent.setup();
    setup({ first: unplacedTicket, layout: failing() });
    await toChoose(u);
    await u.click(
      screen.getByRole("button", { name: "Ask Claude what's in this repo" }),
    );
    await u.click(await screen.findByRole("button", { name: "Cancel import" }));
    expect(await screen.findByText("the library")).toBeVisible();
  });
});

describe("Import: Ask Claude again", () => {
  it("replays the stored source, then asks Claude whatever the setting says", async () => {
    const { calls, args } = setup({
      first: unplacedTicket,
      url: "/import?again=src-1",
    });
    expect(await screen.findByText(/Found 3 artifacts/)).toBeVisible();
    expect(args.library_preview_stored).toEqual({ sourceId: "src-1" });
    await waitFor(() => expect(calls).toContain("library_preview_layout"));
    expect(calls).not.toContain("library_preview");
    expect(await screen.findByText("suggested by Claude")).toBeVisible();
  });

  it("shows why the stored source could not be read", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "library_preview_stored") throw new Error("source is gone");
        return null;
      },
      { shouldMockEvents: true },
    );
    render(
      <MemoryRouter initialEntries={["/import?again=src-1"]}>
        <Routes>
          <Route path="/import" element={<ImportScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText(/source is gone/)).toBeVisible();
    expect(within(document.body).queryByText(/Found/)).toBeNull();
  });

  it("shows that reason whichever source choice is selected", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "library_preview_stored") throw new Error("source is gone");
        return null;
      },
      { shouldMockEvents: true },
    );
    render(
      <MemoryRouter initialEntries={["/import?again=src-1&path=/tmp/x"]}>
        <Routes>
          <Route path="/import" element={<ImportScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByText(/source is gone/)).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent(/source is gone/);
  });

  it("shows a stored-source failure with the git choice selected", () => {
    render(
      <SourceStep
        form={{ choice: "git", url: "", path: "" }}
        failure={{ field: "source", message: "source is gone" }}
        onChange={() => {}}
        onBrowse={() => {}}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent(/source is gone/);
  });

  it("shows the reason with the ~/.claude choice selected", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "library_preview_stored") throw new Error("source is gone");
        return null;
      },
      { shouldMockEvents: true },
    );
    render(
      <MemoryRouter initialEntries={["/import?again=src-1"]}>
        <Routes>
          <Route path="/import" element={<ImportScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      /source is gone/,
    );
    expect(screen.getByRole("button", { name: /~\/\.claude/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });
});
