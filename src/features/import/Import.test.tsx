import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import type { ImportReport } from "../../bindings/ImportReport";
import type { Preview } from "../../bindings/Preview";
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
    },
    alreadyInLibrary: false,
    replacedSecretKeys: [],
    suggestedTags: [],
    ...over,
  };
}

const preview: Preview = {
  source: { kind: "local", path: "/home/me/.claude" },
  diagnostics: [],
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
}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "home_claude_dir":
        return "/home/me/.claude";
      case "library_preview":
        return opts.preview ? opts.preview() : preview;
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

  it("leaves an already-imported row unticked, disabled and labelled", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    const beta = screen.getByRole("checkbox", { name: /beta/ });
    expect(beta).toBeDisabled();
    expect(beta).toHaveAttribute("aria-checked", "false");
    expect(within(beta).getByText("Already in library")).toBeInTheDocument();
    expect(screen.getByText("3 of 3 artifacts selected")).toBeInTheDocument();
  });

  it("ticks one row and all/none per group", async () => {
    const u = user();
    setup({});
    await toChoose(u);
    await u.click(screen.getByRole("checkbox", { name: /alpha/ }));
    expect(screen.getByText("2 of 3 artifacts selected")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Select all Skills" }));
    expect(screen.getByText("3 of 3 artifacts selected")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Select none Skills" }));
    expect(screen.getByText("1 of 3 artifacts selected")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /beta/ })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("goes Back from Choose and keeps the choices", async () => {
    const u = user();
    const calls = setup({});
    await toChoose(u);
    await u.click(screen.getByRole("checkbox", { name: /alpha/ }));
    await u.click(screen.getByRole("button", { name: "Back" }));
    expect(
      screen.getByRole("button", { name: /Your ~\/\.claude/ }),
    ).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("2 of 3 artifacts selected")).toBeInTheDocument();
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
    await u.click(screen.getByRole("checkbox", { name: /gamma/ }));
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await u.click(
      screen.getByRole("button", { name: "Remove docs from alpha" }),
    );
    await u.click(screen.getByRole("button", { name: "Import 2 artifacts" }));
    await screen.findByText("Imported 2 artifacts into your library");
    const call = calls.find((c) => c.cmd === "library_import");
    expect(call?.args).toEqual({
      preview,
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

  it("selects all, none and only the artifacts without errors scan-wide", async () => {
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
    expect(screen.getByText("4 of 4 artifacts selected")).toBeInTheDocument();
    await u.click(screen.getByRole("button", { name: "Only without errors" }));
    expect(screen.getByText("3 of 4 artifacts selected")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: /delta/ })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    await u.click(screen.getByRole("button", { name: "Select all" }));
    await u.click(screen.getByRole("button", { name: "Select none" }));
    expect(screen.getByText("0 of 4 artifacts selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await u.click(screen.getByRole("button", { name: "Select all" }));
    expect(screen.getByText("4 of 4 artifacts selected")).toBeInTheDocument();
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
    await u.type(screen.getByLabelText("New tag for alpha"), "Mine{Enter}");
    expect(screen.getByText("Mine")).toBeInTheDocument();
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
