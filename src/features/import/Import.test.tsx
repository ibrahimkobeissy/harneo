import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { ImportReport } from "../../bindings/ImportReport";
import type { Preview } from "../../bindings/Preview";
import type { TagWithCount } from "../../bindings/TagWithCount";
import { ImportScreen } from ".";
import { defaultTagColor } from "./tagColor";

function candidate(
  name: string,
  over: Partial<Candidate> = {},
  kind: Candidate["artifact"]["kind"] = "skill",
): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind,
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
    suggestedTags: [],
    copies: [],
    audit: {
      score: 100,
      tier: "trusted",
      findings: [],
      reach: { files: [], network: [], shell: [], env: [], protects: 0 },
      dismissed: [],
      confirmation: null,
    },
    ...over,
  };
}

const preview: Preview = {
  source: { kind: "local", path: "/home/me/.claude" },
  diagnostics: [],
  unplaced: [],
  unplacedOverflow: 0,
  layoutIgnored: [],
  candidates: [
    candidate("alpha", { suggestedTags: ["review", "docs"] }),
    candidate("beta", { alreadyInLibrary: true }),
    candidate("gamma", {
      artifact: {
        ...candidate("gamma").artifact,
        diagnostics: [
          { severity: "warning", code: "skill.x", message: "m", file: "f" },
        ],
      },
    }),
    candidate(
      "settings",
      { replacedSecretKeys: ["env.API_TOKEN", "headers.Authorization"] },
      "settings",
    ),
  ],
};

function LibraryProbe() {
  const { search } = useLocation();
  return <h1>Library page{search && `: ${search}`}</h1>;
}

interface Call {
  cmd: string;
  args: unknown;
}

function setup(opts: {
  preview?: () => Preview | Promise<Preview>;
  report?: ImportReport;
  state?: unknown;
  picked?: string | null;
  failImport?: string;
  libraryTags?: TagWithCount[];
}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview":
        return opts.preview ? opts.preview() : { ...preview, token: "tok-1" };
      case "library_import":
        if (opts.failImport) throw new Error(opts.failImport);
        return (
          opts.report ?? {
            sourceId: "src1",
            imported: ["id-alpha", "id-gamma", "id-settings"],
            alreadyInLibrary: [],
            skippedFiles: [],
          }
        );
      case "tags_list":
        return opts.libraryTags ?? [];
      case "tag_create":
        return { id: 99, ...(args as object) };
      case "plugin:dialog|open":
        return opts.picked ?? null;
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={[{ pathname: "/import", state: opts.state }]}>
      <Routes>
        <Route path="/import" element={<ImportScreen />} />
        <Route path="/library" element={<LibraryProbe />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

const user = () => userEvent.setup();

async function toChoose(u: ReturnType<typeof user>) {
  await u.click(screen.getByRole("button", { name: "Continue" }));
  await screen.findByText(/Found 4 artifacts/);
}

describe("Import screen", () => {
  it("starts on the source step with three choices and the stepper", () => {
    setup({});
    expect(
      screen.getByRole("heading", { level: 1, name: "Import" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Source").closest("li")).toHaveAttribute(
      "aria-current",
      "step",
    );
    expect(
      screen.getByRole("button", { name: /Your ~\/\.claude/ }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("button", { name: /A git URL/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /A folder/ }),
    ).toBeInTheDocument();
  });

  it("shows the git hint and the folder field with Browse", async () => {
    const u = user();
    setup({ picked: "/work/harness" });
    await u.click(screen.getByRole("button", { name: /A git URL/ }));
    expect(
      screen.getByText("github.com/owner/repo/tree/main/skills"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await u.click(screen.getByRole("button", { name: /A folder/ }));
    await u.click(screen.getByRole("button", { name: /Browse/ }));
    await waitFor(() =>
      expect(screen.getByLabelText("Folder")).toHaveValue("/work/harness"),
    );
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("scans ~/.claude, then lists candidates by kind with counts", async () => {
    const u = user();
    let release: (p: Preview) => void = () => undefined;
    const calls = setup({
      preview: () => new Promise<Preview>((r) => (release = r)),
    });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByText(/Scanning ~\/\.claude…/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Looking for the 10 artifact types/),
    ).toBeInTheDocument();
    release(preview);
    await screen.findByText(/Found 4 artifacts/);
    expect(calls.find((c) => c.cmd === "library_preview")?.args).toEqual({
      source: { kind: "local", path: "/home/me/.claude" },
      includeManaged: false,
    });
    const skills = screen.getByRole("region", { name: "Skills, 3" });
    expect(within(skills).getByText("alpha")).toBeInTheDocument();
    expect(within(skills).getByText("skills/gamma")).toBeInTheDocument();
    expect(within(skills).getByText("About alpha")).toBeInTheDocument();
    expect(within(skills).getByText("1 warning")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Settings, 1" }),
    ).toBeInTheDocument();
  });

  it("shows an already-imported row as not importable, without an X", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    const skills = screen.getByRole("region", { name: "Skills, 3" });
    expect(within(skills).getByText("Already in library")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Exclude beta" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("3 to import, 0 excluded")).toBeInTheDocument();
  });

  it("excludes one row into Excluded (N) and restores it", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Exclude alpha" }));
    expect(screen.getByText("2 to import, 1 excluded")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Skills, 2" }),
    ).toBeInTheDocument();
    const gone = screen.getByRole("region", { name: "Excluded, 1" });
    expect(within(gone).getByText("alpha")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Restore alpha" }));
    expect(screen.getByText("3 to import, 0 excluded")).toBeInTheDocument();
    expect(
      screen.queryByRole("region", { name: /Excluded/ }),
    ).not.toBeInTheDocument();
  });

  it("moves focus to the next row's X after excluding", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Exclude alpha" }));
    expect(screen.getByRole("button", { name: "Exclude gamma" })).toHaveFocus();
  });

  it("excludes and restores per group and scan-wide", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Exclude all Skills" }));
    expect(screen.getByText("1 to import, 2 excluded")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Skills, 1" }),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Exclude all" }));
    expect(screen.getByText("0 to import, 3 excluded")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await u.click(screen.getByRole("button", { name: "Restore all" }));
    expect(screen.getByText("3 to import, 0 excluded")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Exclude alpha" }));
    await u.click(screen.getByRole("button", { name: "Restore all Skills" }));
    expect(screen.getByText("3 to import, 0 excluded")).toBeInTheDocument();
  });

  it("goes Back from Choose and keeps the exclusions", async () => {
    const u = user();
    const calls = setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Exclude alpha" }));
    await u.click(screen.getByRole("button", { name: "Back" }));
    expect(
      screen.getByRole("button", { name: /Your ~\/\.claude/ }),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("2 to import, 1 excluded")).toBeInTheDocument();
    expect(calls.filter((c) => c.cmd === "library_preview")).toHaveLength(1);
  });

  it("reviews tags and lists secrets by key only", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("Review").closest("li")).toHaveAttribute(
      "aria-current",
      "step",
    );
    expect(screen.getByText("review")).toBeInTheDocument();
    expect(screen.getByText("env.API_TOKEN")).toBeInTheDocument();
    expect(screen.getByText("headers.Authorization")).toBeInTheDocument();
    expect(screen.getByText(/\$\{KEY\}/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Import 3 artifacts" }),
    ).toBeEnabled();
  });

  it("calls library_import with the ticked ids and the tags left", async () => {
    const u = user();
    const calls = setup({
      report: {
        sourceId: "src1",
        imported: ["id-alpha", "id-settings"],
        alreadyInLibrary: ["id-gamma"],
        skippedFiles: [
          {
            artifactId: "id-alpha",
            file: "skills/alpha/token.txt",
            reason: "secretFile",
          },
        ],
      },
    });
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Exclude gamma" }));
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(
      screen.getByRole("button", { name: "Remove docs from alpha" }),
    );
    await u.click(screen.getByRole("button", { name: "Import 2 artifacts" }));
    await screen.findByText("Imported 2 artifacts into your library");
    const call = calls.find((c) => c.cmd === "library_import");
    // Only the backend's token stands for the preview; no preview goes back.
    expect(call?.args).toEqual({
      token: "tok-1",
      selection: ["id-alpha", "id-settings"],
      tags: { "id-alpha": ["review"], "id-settings": [] },
    });
    expect(screen.getByText("1 file not copied")).toBeInTheDocument();
    expect(screen.getByText(/looks like a secret file/)).toBeInTheDocument();
    await u.click(screen.getByRole("link", { name: /Open the Library/ }));
    expect(screen.getByText("Library page: ?source=src1")).toBeInTheDocument();
  });

  it("shows the already-in-library count after an import", async () => {
    const u = user();
    setup({
      report: {
        sourceId: "src1",
        imported: [],
        alreadyInLibrary: ["id-alpha"],
        skippedFiles: [],
      },
    });
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    expect(
      await screen.findByText(/1 artifact already in the library/),
    ).toBeInTheDocument();
  });

  it("keeps the review open and says nothing was imported when the import fails", async () => {
    const u = user();
    setup({ failImport: "this git preview is no longer available" });
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Nothing was imported.",
    );
    expect(screen.getByRole("button", { name: /^Import/ })).toBeEnabled();
  });

  it.each([
    ["git is not installed, or is not on the PATH", /Git is not installed/],
    [
      "cannot reach the git repository https://x/y: it does not exist",
      /Repository not found or unreachable/,
    ],
    [
      "https://x/y needs credentials or does not exist; Harneo never",
      /Authentication required/,
    ],
    [
      "the repository has no branch or tag named dev",
      /Unknown branch or tag “dev”/,
    ],
    ["the repository has no folder skills", /no folder “skills”/],
    ["git did not finish within 60 s and was stopped", /took too long/],
    ["no artifacts found in github.com/o/r", /No artifacts found in/],
  ])(
    "shows the git error %s under the URL field",
    async (message, expected) => {
      const u = user();
      setup({
        preview: () => {
          throw new Error(message);
        },
      });
      await u.click(screen.getByRole("button", { name: /A git URL/ }));
      await u.type(screen.getByLabelText("Repository URL"), "github.com/o/r");
      await u.click(screen.getByRole("button", { name: "Continue" }));
      const alert = await screen.findByRole(
        message.startsWith("no artifacts") ? "status" : "alert",
      );
      expect(alert).toHaveTextContent(expected);
      expect(screen.getByLabelText("Repository URL")).toHaveValue(
        "github.com/o/r",
      );
    },
  );

  it("sends the typed git URL as is, for the app to split", async () => {
    const u = user();
    const calls = setup({});
    await u.click(screen.getByRole("button", { name: /A git URL/ }));
    await u.type(
      screen.getByLabelText("Repository URL"),
      "github.com/o/r/tree/main/skills",
    );
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 4 artifacts/);
    expect(calls.find((c) => c.cmd === "library_preview")?.args).toEqual({
      source: {
        kind: "git",
        url: "github.com/o/r/tree/main/skills",
        branch: null,
        subfolder: null,
      },
      includeManaged: false,
    });
  });

  it("shows a folder error under the folder field and expands ~", async () => {
    const u = user();
    const calls = setup({
      preview: () => {
        throw new Error("cannot scan ~/nope: not a folder");
      },
    });
    await u.click(screen.getByRole("button", { name: /A folder/ }));
    await u.type(screen.getByLabelText("Folder"), "~/nope");
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Folder not found: ~/nope",
    );
    expect(calls.find((c) => c.cmd === "library_preview")?.args).toEqual({
      source: { kind: "local", path: "/home/me/nope" },
      includeManaged: false,
    });
  });

  it("shows the nothing-found state when a scan comes back empty", async () => {
    const u = user();
    setup({ preview: () => ({ ...preview, candidates: [] }) });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "No artifacts found in ~/.claude.",
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("has the first-run variant with a callout and Skip for now", async () => {
    const u = user();
    setup({ state: { firstRun: true } });
    expect(
      screen.getByRole("heading", {
        level: 1,
        name: "Import your first artifacts",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Nothing in your\s+projects or/),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Skip for now" }));
    expect(screen.getByText("Library page")).toBeInTheDocument();
  });

  it("opens a row's diagnostics with code, message and file", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    expect(screen.queryByText("skill.x")).not.toBeInTheDocument();
    await u.click(
      screen.getByRole("button", { name: "Show diagnostics of gamma" }),
    );
    expect(screen.getByText("skill.x")).toBeInTheDocument();
    expect(screen.getByText("m")).toBeInTheDocument();
    expect(screen.getByText("f")).toBeInTheDocument();
    await u.click(
      screen.getByRole("button", { name: "Hide diagnostics of gamma" }),
    );
    expect(screen.queryByText("skill.x")).not.toBeInTheDocument();
  });

  it("excludes the rows with errors and puts them back", async () => {
    const u = user();
    const broken = candidate("delta", {
      artifact: {
        ...candidate("delta").artifact,
        diagnostics: [
          { severity: "error", code: "skill.bad", message: "m", file: "f" },
        ],
      },
    });
    setup({
      preview: () => ({
        ...preview,
        candidates: [...preview.candidates, broken],
      }),
    });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 5 artifacts/);
    expect(screen.getByText("4 to import, 0 excluded")).toBeInTheDocument();
    const btn = screen.getByRole("button", {
      name: "Exclude rows with errors",
    });
    await u.click(btn);
    expect(screen.getByText("3 to import, 1 excluded")).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Excluded, 1" }),
    ).toBeInTheDocument();
    await u.click(btn);
    expect(screen.getByText("4 to import, 0 excluded")).toBeInTheDocument();
  });

  it("collapses and expands groups, and starts large ones collapsed", async () => {
    const u = user();
    const many = Array.from({ length: 21 }, (_, i) =>
      candidate(`cmd${i}`, {}, "command"),
    );
    setup({
      preview: () => ({
        ...preview,
        candidates: [...preview.candidates, ...many],
      }),
    });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 25 artifacts/);
    const commands = screen.getByRole("region", { name: "Commands, 21" });
    expect(within(commands).queryByText("cmd0")).not.toBeInTheDocument();
    expect(screen.getByText("alpha")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Expand Commands" }));
    expect(within(commands).getByText("cmd0")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Collapse all" }));
    expect(screen.queryByText("alpha")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Expand Skills" }),
    ).toHaveAttribute("aria-expanded", "false");
    await u.click(screen.getByRole("button", { name: "Expand all" }));
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("cmd20")).toBeInTheDocument();
  });

  it("labels origins and excludes Claude-provided rows", async () => {
    const u = user();
    const withOrigin = (
      name: string,
      origin: Candidate["artifact"]["origin"],
    ) =>
      candidate(name, {
        artifact: { ...candidate(name).artifact, origin },
      });
    setup({
      preview: () => ({
        ...preview,
        candidates: [
          ...preview.candidates,
          withOrigin("synced", { type: "claudeSynced" }),
          withOrigin("official", {
            type: "marketplace",
            name: "claude-plugins-official",
            repo: null,
            official: true,
          }),
          withOrigin("third", {
            type: "marketplace",
            name: "acme",
            repo: null,
            official: false,
          }),
          withOrigin("mod", { type: "devMod" }),
        ],
      }),
    });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 8 artifacts/);
    expect(screen.getAllByText("Yours").length).toBeGreaterThan(0);
    expect(
      screen.getByText("Synced from your Claude account"),
    ).toBeInTheDocument();
    expect(screen.getByText("Anthropic marketplace")).toBeInTheDocument();
    expect(screen.getByText("Marketplace: acme")).toBeInTheDocument();
    expect(screen.getByText("Mod in development")).toBeInTheDocument();
    await u.click(
      screen.getByRole("button", { name: "Exclude Claude-provided" }),
    );
    expect(screen.getByText("5 to import, 2 excluded")).toBeInTheDocument();
    const gone = screen.getByRole("region", { name: "Excluded, 2" });
    expect(within(gone).getByText("synced")).toBeInTheDocument();
    expect(within(gone).getByText("official")).toBeInTheDocument();
  });

  it("shows copies and lists the other paths on expand", async () => {
    const u = user();
    setup({
      preview: () => ({
        ...preview,
        candidates: [
          candidate("alpha", {
            copies: ["skills/synced/a_b/alpha", "x/alpha"],
          }),
        ],
      }),
    });
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 1 artifact /);
    expect(screen.queryByText("x/alpha")).not.toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "2 copies" }));
    expect(screen.getByText("skills/synced/a_b/alpha")).toBeInTheDocument();
    expect(screen.getByText("x/alpha")).toBeInTheDocument();
  });

  it("notes the skipped managed folders and re-previews with them included", async () => {
    const u = user();
    const skipped: Preview = {
      ...preview,
      diagnostics: [
        {
          severity: "info",
          code: "scan.managed-skipped",
          message:
            "Claude-managed folder skipped: 46 skill folders not scanned",
          file: "skills/synced",
        },
        {
          severity: "info",
          code: "scan.managed-skipped",
          message: "Claude-managed folder skipped: 3 mod folders not scanned",
          file: "dev-mods",
        },
      ],
    };
    let first = true;
    const calls2 = setup({
      preview: () => {
        if (first) {
          first = false;
          return skipped;
        }
        return preview;
      },
    });
    await toChoose(u);
    expect(
      screen.getByText(
        "46 synced skills and 3 mods in development skipped: Claude manages them",
      ),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Include them" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Include them" }),
      ).not.toBeInTheDocument(),
    );
    const previews = calls2.filter((c) => c.cmd === "library_preview");
    expect(previews).toHaveLength(2);
    expect(previews[1]?.args).toEqual({
      source: { kind: "local", path: "/home/me/.claude" },
      includeManaged: true,
    });
  });

  it("tints suggested tags and adds a tag to the import", async () => {
    const u = user();
    const calls = setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("review").closest("span")).toHaveAttribute(
      "data-tint",
      defaultTagColor("review"),
    );
    await u.click(screen.getByRole("button", { name: "Add a tag to alpha" }));
    await waitFor(() => expect(screen.getByRole("combobox")).toHaveFocus());
    await u.keyboard("Mine{Enter}");
    expect(screen.getByText("Mine")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Add a tag to alpha" }),
    ).toHaveFocus();
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    await screen.findByText(/Imported/);
    expect(
      (
        calls.find((c) => c.cmd === "library_import")?.args as {
          tags: Record<string, string[]>;
        }
      ).tags["id-alpha"],
    ).toEqual(["review", "docs", "Mine"]);
  });

  it("Tag all tags every importing artifact and rows stay editable", async () => {
    const u = user();
    const calls = setup({
      libraryTags: [{ id: 5n, name: "second-brain", color: "teal", count: 3 }],
    });
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: "Tag all" }));
    await u.click(await screen.findByRole("option", { name: /second-brain/ }));
    for (const n of ["alpha", "gamma", "settings"]) {
      expect(
        screen.getByRole("button", { name: `Remove second-brain from ${n}` }),
      ).toBeInTheDocument();
    }
    await u.click(
      screen.getByRole("button", { name: "Remove second-brain from gamma" }),
    );
    expect(
      screen.queryByRole("button", { name: "Remove second-brain from all" }),
    ).not.toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    await screen.findByText(/Imported/);
    expect(
      (
        calls.find((c) => c.cmd === "library_import")?.args as {
          tags: Record<string, string[]>;
        }
      ).tags,
    ).toEqual({
      "id-alpha": ["review", "docs", "second-brain"],
      "id-gamma": [],
      "id-settings": ["second-brain"],
    });
    expect(calls.some((c) => c.cmd === "tag_create")).toBe(false);
  });

  it("Remove from all drops a tag present on every row", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: "Tag all" }));
    await u.type(screen.getByRole("combobox"), "shared{Enter}");
    expect(screen.getAllByText("shared")).toHaveLength(4);
    await u.click(
      screen.getByRole("button", { name: "Remove shared from all" }),
    );
    expect(screen.queryByText("shared")).not.toBeInTheDocument();
    expect(screen.getByText("review")).toBeInTheDocument();
  });

  it("Remove all tags takes every tag off every row, then turns off", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: "Tag all" }));
    await u.type(screen.getByRole("combobox"), "shared{Enter}");
    const clear = screen.getByRole("button", { name: "Remove all tags" });
    await u.click(clear);
    expect(screen.queryByText("shared")).not.toBeInTheDocument();
    expect(screen.queryByText("review")).not.toBeInTheDocument();
    expect(clear).toBeDisabled();
  });

  it("creates a new Tag all tag with the chosen color before importing", async () => {
    const u = user();
    const calls = setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: "Tag all" }));
    await u.type(screen.getByRole("combobox"), "Fresh");
    await u.click(screen.getByRole("radio", { name: "pink" }));
    await u.click(screen.getByRole("option", { name: /Create/ }));
    expect(screen.getAllByText("Fresh")[0]?.closest("span")).toHaveAttribute(
      "data-tint",
      "pink",
    );
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    await screen.findByText(/Imported/);
    const order = calls.map((c) => c.cmd);
    expect(order.indexOf("tag_create")).toBeGreaterThan(-1);
    expect(order.indexOf("tag_create")).toBeLessThan(
      order.indexOf("library_import"),
    );
    expect(calls.find((c) => c.cmd === "tag_create")?.args).toEqual({
      name: "Fresh",
      color: "pink",
    });
  });

  it("a row's Add tag offers the palette and saves the chosen color", async () => {
    const u = user();
    const calls = setup({
      libraryTags: [{ id: 5n, name: "second-brain", color: "teal", count: 3 }],
    });
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(screen.getByRole("button", { name: "Add a tag to alpha" }));
    await waitFor(() => expect(screen.getByRole("combobox")).toHaveFocus());
    await u.keyboard("Fresh");
    await u.click(screen.getByRole("radio", { name: "pink" }));
    await u.click(screen.getByRole("option", { name: /Create/ }));
    expect(screen.getByText("Fresh").closest("span")).toHaveAttribute(
      "data-tint",
      "pink",
    );
    await u.click(screen.getByRole("button", { name: "Add a tag to alpha" }));
    await u.click(await screen.findByRole("option", { name: /second-brain/ }));
    expect(screen.getAllByText("second-brain")).toHaveLength(1);
    await u.click(screen.getByRole("button", { name: /^Import/ }));
    await screen.findByText(/Imported/);
    expect(
      calls.filter((c) => c.cmd === "tag_create").map((c) => c.args),
    ).toEqual([{ name: "Fresh", color: "pink" }]);
  });

  it("Escape closes a row's tag picker and returns focus to its button", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    const add = screen.getByRole("button", { name: "Add a tag to alpha" });
    await u.click(add);
    await waitFor(() => expect(screen.getByRole("combobox")).toHaveFocus());
    await u.keyboard("{Escape}");
    expect(screen.queryByRole("combobox")).toBeNull();
    expect(add).toHaveFocus();
  });

  it("keeps the progress list valid: only list items inside the list", async () => {
    setup({});
    const steps = await screen.findByRole("list", { name: "Progress" });
    expect(
      Array.from(steps.children).every((child) => child.tagName === "LI"),
    ).toBe(true);
    expect(steps.children).toHaveLength(3);
  });

  it("renders the secrets list without list markers", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      screen.getByRole("list", { name: "Secrets in settings" }),
    ).toBeInTheDocument();
  });
});
