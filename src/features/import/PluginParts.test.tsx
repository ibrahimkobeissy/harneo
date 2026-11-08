import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { PluginHold } from "../../bindings/PluginHold";
import type { Preview } from "../../bindings/Preview";
import { ImportScreen } from ".";
import { FOLD_OVER, PluginParts, partsSummary } from "./PluginRows";

function candidate(
  name: string,
  kind: Candidate["artifact"]["kind"],
  path: string,
  metadata: Record<string, unknown> = {},
  diagnostics: Candidate["artifact"]["diagnostics"] = [],
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
      diagnostics,
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
const loose = candidate("loose", "skill", "skills/loose");
const base: Omit<Preview, "candidates"> = {
  source: { kind: "local", path: "/home/me/kit" },
  diagnostics: [],
  unplaced: [],
  unplacedOverflow: 0,
  layoutIgnored: [],
};

const whole: Preview = {
  ...base,
  candidates: [loose, candidate("toolbox", "plugin", "")],
};
const parts: Preview = {
  ...base,
  candidates: [
    loose,
    candidate("alpha", "skill", "skills/alpha", from),
    candidate("beta", "skill", "skills/beta", from),
    candidate("gamma", "skill", "skills/gamma", from),
    candidate("needy", "skill", "skills/needy", from, [
      {
        severity: "error",
        code: "plugin.part-needs-plugin",
        message: "uses ${CLAUDE_PLUGIN_ROOT}/shared/tool.sh",
        file: "skills/needy/SKILL.md",
      },
    ]),
    candidate("db", "mcp", ".mcp.json#db", from),
  ],
};

interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

function setup(hold: Partial<PluginHold> = {}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview": {
        const unpacked = Array.isArray(a.unpack) && a.unpack.length > 0;
        return { ...(unpacked ? parts : whole), token: "tok" };
      }
      case "library_plugin_holds":
        return [
          { name: "toolbox", folder: "", whole: false, parts: false, ...hold },
        ];
      case "library_import":
        return {
          sourceId: "s",
          imported: [],
          alreadyInLibrary: [],
          skippedFiles: [],
        };
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
        <Route path="/library" element={<h1>Library page</h1>} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

async function toChoose(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found 2 artifacts/);
}

const pick = () => screen.getByRole("radio", { name: "Pick parts" });
const wholeRadio = () => screen.getByRole("radio", { name: "Whole plugin" });

describe("choosing a plugin's parts", () => {
  it("lists the parts nested and ticked, imports only the ticked ones and goes back to the whole plugin", async () => {
    const user = userEvent.setup();
    const calls = setup();
    await toChoose(user);
    expect(wholeRadio()).toBeChecked();

    await user.click(pick());
    await screen.findByRole("checkbox", { name: /alpha/ });
    const previews = calls.filter((c) => c.cmd === "library_preview");
    expect(previews[1]?.args.unpack).toEqual([""]);
    const skills = screen.getByRole("region", { name: /Skills of toolbox, 4/ });
    expect(
      within(within(skills).getByRole("list")).getAllByRole("checkbox"),
    ).toHaveLength(4);
    // Every part that can be picked starts ticked; the one that needs its plugin can't be.
    for (const name of [/alpha/, /beta/, /gamma/]) {
      expect(within(skills).getByRole("checkbox", { name })).toBeChecked();
    }
    expect(
      within(skills).getByRole("checkbox", { name: /needy/ }),
    ).not.toBeChecked();
    expect(
      screen.getByRole("region", { name: /MCP servers of toolbox/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/come only with the whole plugin/)).toBeVisible();
    // The loose skill keeps its own tick, next to the three skills and the MCP server.
    expect(screen.getByText(/5 to import/)).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: /gamma/ }));
    await user.click(screen.getByRole("checkbox", { name: /db/ }));
    expect(screen.getByText(/3 to import/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await user.click(await screen.findByRole("button", { name: /^Import 3/ }));
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "library_import")).toBe(true),
    );
    const imported = calls.find((c) => c.cmd === "library_import");
    expect(imported?.args.selection).toEqual([
      "id-loose",
      "id-alpha",
      "id-beta",
    ]);
  });

  it("restores the single plugin row, ticked as before, when switched back", async () => {
    const user = userEvent.setup();
    const calls = setup();
    await toChoose(user);
    await user.click(pick());
    await screen.findByRole("checkbox", { name: /alpha/ });
    await user.click(wholeRadio());
    await screen.findByRole("button", { name: "Exclude toolbox" });
    expect(screen.queryByRole("checkbox", { name: /alpha/ })).toBeNull();
    expect(screen.getByText(/2 to import/)).toBeInTheDocument();
    expect(wholeRadio()).toBeChecked();
    const last = calls.filter((c) => c.cmd === "library_preview").at(-1);
    expect(last?.args.unpack).toBeUndefined();
  });

  it("shows a part that needs its plugin disabled, with the reason in words", async () => {
    const user = userEvent.setup();
    setup();
    await toChoose(user);
    await user.click(pick());
    const needy = await screen.findByRole("checkbox", { name: /needy/ });
    expect(needy).toHaveAttribute("aria-disabled", "true");
    expect(needy).toHaveAccessibleDescription(
      /uses \$\{CLAUDE_PLUGIN_ROOT\}\/shared\/tool\.sh/,
    );
    expect(screen.getByText(/Stays with the plugin/)).toBeVisible();
    needy.focus();
    expect(needy).toHaveFocus();
    await user.click(needy);
    expect(needy).not.toBeChecked();
    expect(screen.getByText(/5 to import/)).toBeInTheDocument();
  });

  it("makes Pick parts unavailable, with the reason, while the plugin is in the library whole", async () => {
    const user = userEvent.setup();
    const calls = setup({ whole: true });
    await toChoose(user);
    await waitFor(() =>
      expect(pick()).toHaveAttribute("aria-disabled", "true"),
    );
    expect(pick()).toHaveAccessibleDescription(/in the library whole/);
    expect(screen.getByText(/remove the plugin toolbox first/)).toBeVisible();
    await user.click(pick());
    expect(wholeRadio()).toBeChecked();
    expect(calls.filter((c) => c.cmd === "library_preview")).toHaveLength(1);
  });

  it("refuses the whole plugin up front while parts of it are in the library", async () => {
    const user = userEvent.setup();
    setup({ parts: true });
    await toChoose(user);
    await waitFor(() =>
      expect(wholeRadio()).toHaveAttribute("aria-disabled", "true"),
    );
    expect(wholeRadio()).toHaveAccessibleDescription(
      /remove them first to import the plugin toolbox whole/,
    );
    // It starts out of the import and cannot be restored by hand or in bulk.
    await screen.findByText(/1 to import, 1 excluded/);
    expect(
      screen.getByRole("button", { name: "Restore toolbox" }),
    ).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Restore all" }));
    expect(screen.getByText(/1 to import, 1 excluded/)).toBeInTheDocument();
    // Pick parts stays open: the library holds parts, not the whole.
    await user.click(pick());
    await screen.findByRole("checkbox", { name: /alpha/ });
  });
});

describe("excluding a plugin in Pick parts mode (#260)", () => {
  it("takes the plugin out with its ×, lists it under Excluded and restores every part", async () => {
    const user = userEvent.setup();
    const calls = setup();
    await toChoose(user);
    await user.click(pick());
    await screen.findByRole("checkbox", { name: /alpha/ });
    expect(screen.getByText(/5 to import/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Exclude toolbox" }));
    // The plugin and its parts leave the list; only the loose skill is left to import.
    expect(screen.queryByRole("checkbox", { name: /alpha/ })).toBeNull();
    expect(screen.getByText(/1 to import/)).toBeInTheDocument();
    const out = screen.getByRole("region", { name: "Excluded, 1" });
    expect(within(out).getByText("toolbox")).toBeVisible();
    expect(within(out).getByText("4 parts")).toBeVisible();

    await user.click(
      within(out).getByRole("button", { name: "Restore toolbox" }),
    );
    expect(
      await screen.findByRole("checkbox", { name: /alpha/ }),
    ).toBeChecked();
    expect(screen.getByText(/5 to import/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /^Excluded/ })).toBeNull();

    // Unticking every part by hand keeps the plugin in place, still offering its boxes.
    for (const name of [/alpha/, /beta/, /gamma/, /db/])
      await user.click(screen.getByRole("checkbox", { name }));
    expect(screen.getByRole("checkbox", { name: /alpha/ })).not.toBeChecked();
    expect(
      screen.getByRole("button", { name: "Exclude toolbox" }),
    ).toBeVisible();
    expect(calls.some((c) => c.cmd === "library_import")).toBe(false);
  });
});

describe("folding a plugin's parts", () => {
  const many = Array.from({ length: FOLD_OVER + 1 }, (_, i) =>
    candidate(`s${i}`, "skill", `skills/s${i}`, from),
  ).concat([candidate("cmd", "command", "commands/cmd.md", from)]);
  const group = { name: "pack", folder: "", whole: null, parts: many };

  function show(excluded: string[] = [], parts = many) {
    const sets: [string[], boolean][] = [];
    const view = render(
      <PluginParts
        group={{ ...group, parts }}
        excluded={new Set(excluded)}
        onSet={(ids, on) => sets.push([ids, on])}
      />,
    );
    return { sets, view };
  }

  it("starts a big plugin folded, with what it holds in one line", async () => {
    const user = userEvent.setup();
    show(["id-s0"]);
    const toggle = screen.getByRole("button", { name: /10 parts/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(
      screen.getByText("9 of 10 ticked · 9 skills, 1 command"),
    ).toBeVisible();
    expect(screen.queryByRole("checkbox", { name: /^s1/ })).toBeNull();
    await user.click(toggle);
    expect(screen.getByRole("button", { name: "Parts" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("checkbox", { name: /^s1/ })).toBeChecked();
  });

  it("starts a small plugin, or one with a part that needs attention, open", () => {
    const { view } = show([], many.slice(0, 3));
    expect(screen.getByRole("button", { name: "Parts" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    view.unmount();
    const needy = candidate("needy", "skill", "skills/needy", from, [
      {
        severity: "error",
        code: "plugin.part-needs-plugin",
        message: "uses ${CLAUDE_PLUGIN_ROOT}/x",
        file: "skills/needy/SKILL.md",
      },
    ]);
    show([], [...many, needy]);
    expect(screen.getByRole("button", { name: "Parts" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  it("ticks and unticks every pickable part from the plugin and from a kind", async () => {
    const user = userEvent.setup();
    const { sets } = show(["id-s0"]);
    const all = screen.getByRole("checkbox", { name: "All parts of pack" });
    expect(all).toHaveAttribute("aria-checked", "mixed");
    expect((all as HTMLInputElement).indeterminate).toBe(true);
    await user.click(all);
    expect(sets.at(-1)).toEqual([many.map((c) => c.id), true]);
    await user.click(screen.getByRole("button", { name: /10 parts/ }));
    await user.click(
      screen.getByRole("checkbox", { name: "All commands of pack" }),
    );
    expect(sets.at(-1)).toEqual([["id-cmd"], false]);
    await user.click(screen.getByRole("checkbox", { name: /^s1/ }));
    expect(sets.at(-1)).toEqual([["id-s1"], false]);
  });

  it("folds one kind on its own", async () => {
    const user = userEvent.setup();
    show([], many.slice(0, 3).concat([many.at(-1)!]));
    const skills = screen.getByRole("button", { name: /Skills/ });
    await user.click(skills);
    expect(skills).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("checkbox", { name: /^s0/ })).toBeNull();
    expect(screen.getByRole("checkbox", { name: /^cmd/ })).toBeVisible();
  });
});

describe("the folded summary", () => {
  it("names kinds in running text", () => {
    const of = (n: number) => Array.from({ length: n }) as never[];
    expect(
      partsSummary(
        [
          ["Skills", of(2)],
          ["MCP servers", of(1)],
          ["CLAUDE.md", of(1)],
          ["Commands", of(1)],
        ],
        3,
        5,
      ),
    ).toBe("3 of 5 ticked · 2 skills, 1 MCP server, 1 CLAUDE.md, 1 command");
  });
});
