import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { Preview } from "../../bindings/Preview";
import type { WholeReason } from "../../bindings/WholeReason";
import { ImportScreen } from ".";

function candidate(
  name: string,
  kind: Candidate["artifact"]["kind"],
  path: string,
  metadata: Record<string, unknown> = {},
  mustStayWhole?: WholeReason,
): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind,
      name,
      description: `About ${name}`,
      path,
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
      reach: { files: [], network: [], shell: [], env: [], protects: 0 },
      unaudited: [],
      dismissed: [],
      confirmation: null,
    },
    ...(mustStayWhole && { mustStayWhole }),
  };
}

const from = { fromPlugin: { name: "pack", folder: "", marketplace: null } };
const base: Omit<Preview, "candidates"> = {
  source: { kind: "local", path: "/home/me/pack" },
  diagnostics: [],
  unplaced: [],
  unplacedOverflow: 0,
  layoutIgnored: [],
};
const parts: Preview = {
  ...base,
  candidates: [
    candidate("one", "skill", "skills/one", from),
    candidate("two", "skill", "skills/two", from),
  ],
};
const hooks: WholeReason = { type: "wholeOnlyParts", parts: ["hooks", "mods"] };
const wholeHooks: Preview = {
  ...base,
  candidates: [candidate("pack", "plugin", "", {}, hooks)],
};

interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

/** The backend: the default (no unpack, no whole) answers `first`; an unpack answers parts. */
function setup(first: Preview) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview": {
        const unpacked = Array.isArray(a.unpack) && a.unpack.length > 0;
        const asked = Array.isArray(a.whole) && a.whole.length > 0;
        return {
          ...(unpacked ? parts : asked ? wholeHooks : first),
          token: "t",
        };
      }
      case "library_plugin_holds":
        return [{ name: "pack", folder: "", whole: false, parts: false }];
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
  return calls;
}

describe("plugins are unpacked by default", () => {
  it("shows a skills pack as Pick parts with every part ticked and asks with no choice", async () => {
    const user = userEvent.setup();
    const calls = setup(parts);
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByRole("checkbox", { name: /one/ });
    expect(screen.getByRole("radio", { name: "Pick parts" })).toBeChecked();
    const skills = screen.getByRole("region", { name: /Skills of pack, 2/ });
    for (const box of within(skills).getAllByRole("checkbox")) {
      expect(box).toBeChecked();
    }
    const first = calls.find((c) => c.cmd === "library_preview");
    expect(first?.args.unpack).toBeUndefined();
    expect(first?.args.whole).toBeUndefined();
  });

  it("proposes a plugin with hooks whole, says why, and lists what unpacking loses", async () => {
    const user = userEvent.setup();
    const calls = setup(wholeHooks);
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByRole("button", { name: "Exclude pack" });
    expect(screen.getByRole("radio", { name: "Whole plugin" })).toBeChecked();
    expect(
      screen.getByText("Has hooks, mods: they only work inside the plugin."),
    ).toBeVisible();

    await user.click(screen.getByRole("radio", { name: "Pick parts" }));
    await screen.findByRole("checkbox", { name: /one/ });
    expect(screen.getByText(/Lost if unpacked: hooks, mods\./)).toBeVisible();
    const previews = calls.filter((c) => c.cmd === "library_preview");
    expect(previews[1]?.args.unpack).toEqual([""]);

    // Back to whole: the choice is explicit, so the default does not unpack it again.
    await user.click(screen.getByRole("radio", { name: "Whole plugin" }));
    await screen.findByRole("button", { name: "Exclude pack" });
    const last = calls.filter((c) => c.cmd === "library_preview").at(-1);
    expect(last?.args.whole).toEqual([""]);
    expect(last?.args.unpack).toBeUndefined();
  });
});
