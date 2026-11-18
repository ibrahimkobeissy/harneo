import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import { ImportScreen } from ".";
import { splitBlocks } from "./splitBlock";

function candidate(
  name: string,
  kind: Candidate["artifact"]["kind"],
  path: string,
  metadata: Record<string, unknown> = {},
): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind,
      name,
      description: null,
      path,
      files: [],
      linkedFiles: [],
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
      reach: { files: [], network: [], shell: [], env: [], protects: 0 },
      unaudited: [],
      dismissed: [],
      confirmation: null,
    },
  };
}

const from = { fromPlugin: { name: "toolbox", folder: "", marketplace: null } };
const alpha = candidate("alpha", "skill", "skills/alpha", from);
const build = candidate("build", "command", "commands/build.md", from);

function ticket(over: Partial<PreviewTicket> = {}): PreviewTicket {
  return {
    token: "tok",
    source: { kind: "local", path: "/home/me/kit" },
    diagnostics: [],
    unplaced: [],
    unplacedOverflow: 0,
    layoutIgnored: [],
    partNeeds: [],
    notNeeded: [],
    partRefs: [],
    candidates: [alpha, build],
    ...over,
  } as PreviewTicket;
}

const wholeTicket = ticket({
  candidates: [candidate("toolbox", "plugin", "")],
});
const complete = ticket({
  needsBy: "claude",
  partNeeds: [
    { path: "skills/alpha", needs: [] },
    { path: "commands/build.md", needs: [] },
  ],
});
const incomplete = ticket({
  needsBy: "claude",
  partNeeds: [{ path: "skills/alpha", needs: [] }],
});

describe("splitBlocks", () => {
  const none = new Set<string>();

  it("names the three cases", () => {
    expect(splitBlocks(ticket(), none, null)[0]?.why).toEqual({
      kind: "notAsked",
    });
    expect(splitBlocks(complete, none, "boom")[0]?.why).toEqual({
      kind: "failed",
      message: "boom",
    });
    expect(splitBlocks(incomplete, none, null)[0]?.why).toEqual({
      kind: "incomplete",
      missing: ["/build"],
    });
  });

  it("lets a fully answered plugin, a dropped part and a whole plugin through", () => {
    expect(splitBlocks(complete, none, null)).toEqual([]);
    expect(splitBlocks(incomplete, new Set(["id-build"]), null)).toEqual([]);
    expect(
      splitBlocks(ticket(), new Set(["id-alpha", "id-build"]), null),
    ).toEqual([]);
    expect(splitBlocks(wholeTicket, none, null)).toEqual([]);
  });
});

function setup(first: PreviewTicket, layout: () => PreviewTicket) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview": {
        const unpacked = Array.isArray(a.unpack) && a.unpack.length > 0;
        const wholeList = Array.isArray(a.whole) && a.whole.length > 0;
        return wholeList && !unpacked ? wholeTicket : first;
      }
      case "library_preview_layout":
        return layout();
      case "library_plugin_holds":
        return [{ name: "toolbox", folder: "", whole: false, parts: false }];
      case "import_layout_setting_get":
        return false;
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
        <Route path="/library" element={<p>the library</p>} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

async function toChoose(u: ReturnType<typeof userEvent.setup>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found \d+ artifacts?/);
}

const cont = () => screen.getByRole("button", { name: "Continue" });

describe("Import: a split plugin needs Claude's answer", () => {
  it("blocks when Claude was not asked, then unblocks once Ask Claude again gives a full answer", async () => {
    const u = userEvent.setup();
    const calls = setup(ticket(), () => complete);
    await toChoose(u);
    expect(await screen.findByText(/Claude was not asked/)).toBeVisible();
    expect(cont()).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: "Continue with what was found" }),
    ).toBeNull();
    await u.click(screen.getByRole("button", { name: "Ask Claude again" }));
    await waitFor(() =>
      expect(calls.map((c) => c.cmd)).toContain("library_preview_layout"),
    );
    await waitFor(() => expect(cont()).toBeEnabled());
    expect(screen.queryByText(/Claude was not asked/)).toBeNull();
  });

  it("blocks when the call failed, with the error", async () => {
    const u = userEvent.setup();
    setup(ticket(), () => {
      throw new Error("claude's answer could not be used: no list");
    });
    await toChoose(u);
    await u.click(
      await screen.findByRole("button", { name: "Ask Claude again" }),
    );
    expect(
      await screen.findByText(/Claude's call failed.*no list/),
    ).toBeVisible();
    expect(cont()).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Continue with what was found" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Take the whole plugin instead" }),
    ).toBeEnabled();
  });

  it("blocks an incomplete answer and lists the parts it misses", async () => {
    const u = userEvent.setup();
    setup(incomplete, () => incomplete);
    await toChoose(u);
    expect(
      await screen.findByText(/Claude's answer doesn't cover `\/build`/),
    ).toBeVisible();
    expect(cont()).toBeDisabled();
  });

  it("takes the whole plugin instead and unblocks", async () => {
    const u = userEvent.setup();
    const calls = setup(incomplete, () => incomplete);
    await toChoose(u);
    await u.click(
      await screen.findByRole("button", {
        name: "Take the whole plugin instead",
      }),
    );
    await waitFor(() => expect(cont()).toBeEnabled());
    const last = calls.filter((c) => c.cmd === "library_preview").at(-1);
    expect(last?.args.whole).toEqual([""]);
    expect(screen.queryByText(/doesn't cover/)).toBeNull();
  });

  it("lets a fully answered plugin through", async () => {
    const u = userEvent.setup();
    setup(complete, () => complete);
    await toChoose(u);
    expect(cont()).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: "Ask Claude again" }),
    ).toBeNull();
  });
});
