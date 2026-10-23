import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { Comparison } from "../../bindings/Comparison";
import type { Row } from "../../bindings/Row";
import { RefitScreen } from ".";

const b64 = (s: string) => btoa(s);

function profile(id: number, name: string) {
  return {
    id,
    name,
    description: `About ${name}`,
    version: 2,
    counts: [],
    fileCount: 5,
    projectCount: 1,
    updatedAt: 1,
  };
}

function row(over: Partial<Row> & Pick<Row, "item">): Row {
  return {
    kind: "both",
    paths: [over.item],
    inProject: "differs",
    inProfile: "differs",
    defaultAction: "replace",
    options: ["keep", "replace"],
    loss: null,
    ...over,
  };
}

const rows: Row[] = [
  row({
    item: "CLAUDE.md",
    loss: {
      lines: ["## Deploy", "make deploy"],
      count: 14,
      unit: "lines",
    },
  }),
  row({
    item: ".claude/settings.json",
    loss: { lines: ['"Bash(make:*)"'], count: 1, unit: "permissionRules" },
  }),
  row({
    item: ".claude/skills/tdd",
    loss: { lines: ["old line"], count: 1, unit: "lines" },
  }),
  row({
    item: ".claude/agents/reviewer.md",
    kind: "same",
    inProject: "same",
    inProfile: "same",
    defaultAction: "keep",
    options: ["keep"],
  }),
  row({
    item: ".claude/commands/deploy.md",
    kind: "projectOnly",
    inProject: "exists",
    inProfile: "absent",
    defaultAction: "keep",
    options: ["keep", "delete"],
    loss: { lines: ["deploy it", "carefully"], count: 5, unit: "lines" },
  }),
  row({
    item: "HARNESS.md",
    kind: "profileOnly",
    inProject: "absent",
    inProfile: "new",
    defaultAction: "add",
    options: ["add", "skip"],
  }),
];

function comparison(over: Partial<Comparison> = {}): Comparison {
  return {
    target: "/work/site",
    profileId: 1n,
    profileName: "Web",
    profileVersion: 2,
    rows,
    nothingToChange: false,
    ...over,
  };
}

const plan = {
  target: "/work/site",
  profileId: 1,
  profileName: "Web",
  profileVersion: 2,
  reason: "Refit Web v2",
  backupCount: 2,
  kind: "refit",
  editedInProject: [],
  restored: [],
  writes: [
    {
      path: "CLAUDE.md",
      action: "replace",
      bytes: b64("# new\n"),
      diff: [
        {
          oldStart: 1,
          newStart: 1,
          lines: [
            { kind: "removed", text: "## Deploy" },
            { kind: "added", text: "@HARNESS.md" },
          ],
        },
      ],
      artifact: null,
    },
    {
      path: ".claude/commands/deploy.md",
      action: "delete",
      bytes: null,
      diff: null,
      artifact: null,
    },
    {
      path: "HARNESS.md",
      action: "new",
      bytes: b64("# Harness\n"),
      diff: null,
      artifact: null,
    },
    {
      path: ".claude/agents/reviewer.md",
      action: "keep",
      bytes: null,
      diff: null,
      artifact: null,
    },
  ],
};

interface Opts {
  comparison?: Comparison;
  compareError?: string;
  planError?: string;
  applyError?: string;
  restoreError?: string;
  listError?: string;
  projectsFail?: boolean;
  profiles?: ReturnType<typeof profile>[];
  backupId?: number | null;
  picked?: string | null;
  pickFails?: boolean;
  url?: string;
}

function Where() {
  const l = useLocation();
  return <p data-testid="where">{l.pathname + l.search}</p>;
}

function setup(opts: Opts = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "profiles_list":
        if (opts.listError) throw new Error(opts.listError);
        return opts.profiles ?? [profile(1, "Web"), profile(2, "API")];
      case "projects_list":
        if (opts.projectsFail) throw new Error("db");
        return [
          {
            id: 9,
            path: "/work/site",
            name: "site",
            profileId: 2,
            profileName: "API",
            profileVersion: 1,
            appliedAt: 1,
            fileCount: 3,
          },
        ];
      case "plugin:dialog|open":
        if (opts.pickFails) throw new Error("no dialog");
        return opts.picked === undefined ? "/work/site" : opts.picked;
      case "home_claude_dir":
        return "/home/me/.claude";
      case "refit_compare":
        if (opts.compareError) throw new Error(opts.compareError);
        return opts.comparison ?? comparison();
      case "refit_plan":
        if (opts.planError) throw new Error(opts.planError);
        return plan;
      case "install_apply":
        if (opts.applyError) throw new Error(opts.applyError);
        return {
          projectId: 9,
          written: 2,
          deleted: 1,
          backedUp: 2,
          backupId: opts.backupId === undefined ? 5 : opts.backupId,
          backupFolder: null,
        };
      case "backup_restore":
        if (opts.restoreError) throw new Error(opts.restoreError);
        return { projectId: 9, written: 2, deleted: 0, backedUp: 2 };
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={[opts.url ?? "/refit"]}>
      <Routes>
        <Route path="/refit" element={<RefitScreen />} />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

const cont = () => screen.getByRole("button", { name: "Continue" });

async function toProfile(path = "/work/site") {
  await userEvent.type(screen.getByLabelText("Project folder"), path);
  await userEvent.click(cont());
}

async function toCompare(path?: string) {
  await toProfile(path);
  await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
  await userEvent.click(cont());
  await screen.findByRole("table");
}

async function toPreview() {
  await toCompare();
  await userEvent.click(
    screen.getByRole("button", { name: "Preview changes" }),
  );
  await screen.findByText(/Dry run/);
}

describe("Refit flow", () => {
  it("lists registered projects and picks one", async () => {
    setup();
    const known = await screen.findByRole("button", {
      name: /\/work\/site.*registered · API v1/,
    });
    await userEvent.click(known);
    expect(screen.getByLabelText("Project folder")).toHaveValue("/work/site");
    // The registered project's current profile comes preselected.
    await userEvent.click(cont());
    expect(
      await screen.findByRole("button", { name: /API.*current profile/ }),
    ).toHaveAttribute("aria-pressed", "true");
  });

  it("presets the folder from ?path= and needs one to continue", async () => {
    setup({ url: "/refit?path=%2Fwork%2Fsite" });
    expect(screen.getByLabelText("Project folder")).toHaveValue("/work/site");
  });

  it("disables Continue without a folder; Browse fills it; cancelled and failing dialogs keep it", async () => {
    setup({ picked: null });
    expect(cont()).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    expect(screen.getByLabelText("Project folder")).toHaveValue("");
  });

  it("Browse fills in the chosen folder", async () => {
    setup();
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    await waitFor(() =>
      expect(screen.getByLabelText("Project folder")).toHaveValue("/work/site"),
    );
  });

  it("survives a failing dialog and a failing project list", async () => {
    setup({ pickFails: true, projectsFail: true });
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    expect(screen.getByLabelText("Project folder")).toHaveValue("");
    expect(screen.queryByText("Folders with a harness")).toBeNull();
  });

  it("compares with the chosen profile, sends the choices and walks to Done", async () => {
    const calls = setup();
    await toCompare();
    expect(calls.find((c) => c.cmd === "refit_compare")?.args).toMatchObject({
      target: "/work/site",
      profileId: 1,
    });
    // Top loss callout lists only the two critical files.
    const alert = screen.getByRole("alert");
    expect(alert).toHaveTextContent("CLAUDE.md: 14 lines will be lost");
    expect(alert).toHaveTextContent(
      "settings.json: 1 permission rule will be lost",
    );
    expect(alert).not.toHaveTextContent("tdd");
    // Defaults: replace 3, add 1, delete 0, keep 2, skip 0; 3 backed up.
    expect(
      screen.getByText(/Replace 3 · add 1 · delete 0 · keep 2 · skip 0\./),
    ).toBeInTheDocument();
    expect(screen.getByText("3 files")).toBeInTheDocument();
    // Keep CLAUDE.md: its loss and the callout entry go away.
    const group = screen.getByRole("group", { name: "Action for CLAUDE.md" });
    await userEvent.click(within(group).getByRole("button", { name: "Keep" }));
    expect(screen.getByRole("alert")).not.toHaveTextContent("CLAUDE.md:");
    // Delete the project-only command.
    await userEvent.click(
      within(
        screen.getByRole("group", {
          name: "Action for .claude/commands/deploy.md",
        }),
      ).getByRole("button", { name: "Delete" }),
    );
    expect(screen.getByText(/delete 1/)).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Preview changes" }),
    );
    await screen.findByText(/Dry run/);
    expect(calls.find((c) => c.cmd === "refit_plan")?.args).toMatchObject({
      target: "/work/site",
      profileId: 1,
      choices: {
        "CLAUDE.md": "keep",
        ".claude/commands/deploy.md": "delete",
      },
    });
    // Kept and skipped files are not listed; the replaced one shows its diff.
    expect(screen.getByText(/3 files will change/)).toBeInTheDocument();
    expect(screen.getByText("replace")).toBeInTheDocument();
    expect(screen.getByText("delete")).toBeInTheDocument();
    expect(screen.getByText("new")).toBeInTheDocument();
    expect(screen.queryByText("reviewer.md")).toBeNull();
    expect(screen.getByText("@HARNESS.md")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /deploy\.md/ }));
    expect(screen.getByText(/removed from the project/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Apply refit/ }));
    expect(
      await screen.findByText("Refitted site with Web v2"),
    ).toBeInTheDocument();
    expect(screen.getByText(/2 originals backed up/)).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Go to projects" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent("/projects");
  });

  it("shows and hides the lost lines, red for CLAUDE.md and plain for others", async () => {
    setup();
    await toCompare();
    expect(screen.queryByText("− ## Deploy")).toBeNull();
    await userEvent.click(
      screen.getByRole("button", { name: "Show lost lines of CLAUDE.md" }),
    );
    expect(screen.getByText("− ## Deploy")).toBeInTheDocument();
    expect(screen.getByText("… 12 more")).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Hide lost lines of CLAUDE.md" }),
    );
    expect(screen.queryByText("− ## Deploy")).toBeNull();
    // A non-critical row shows its count without the red callout entry.
    expect(screen.getAllByText("1 line will be lost")).toHaveLength(1);
    // Show again and open another to exercise the set.
    await userEvent.click(
      screen.getByRole("button", { name: "Show lost lines of CLAUDE.md" }),
    );
    await userEvent.click(
      screen.getByRole("button", {
        name: "Show lost lines of .claude/skills/tdd",
      }),
    );
    expect(screen.getByText("− old line")).toBeInTheDocument();
  });

  it("a project-only loss appears when Delete is chosen", async () => {
    setup();
    await toCompare();
    expect(screen.queryByText("5 lines will be lost")).toBeNull();
    await userEvent.click(
      within(
        screen.getByRole("group", {
          name: "Action for .claude/commands/deploy.md",
        }),
      ).getByRole("button", { name: "Delete" }),
    );
    expect(screen.getByText("5 lines will be lost")).toBeInTheDocument();
  });

  it("sorts the table by each column and toggles the direction", async () => {
    setup();
    await toCompare();
    const items = () =>
      screen
        .getAllByRole("row")
        .slice(1)
        .map((r) => within(r).getAllByRole("cell")[0]?.textContent ?? "");
    expect(items()[0]).toContain(".claude/agents/reviewer.md");
    const sortBy = (name: string) =>
      userEvent.click(screen.getByRole("button", { name }));
    await sortBy("File");
    expect(items()[0]).toContain("HARNESS.md");
    await sortBy("In project");
    await sortBy("In Web");
    await sortBy("Action");
    expect(
      screen.getByRole("columnheader", { name: /Action/ }),
    ).toHaveAttribute("aria-sort", "ascending");
    await sortBy("Action");
    expect(
      screen.getByRole("columnheader", { name: /Action/ }),
    ).toHaveAttribute("aria-sort", "descending");
  });

  it("shows Nothing to change with Back to projects", async () => {
    setup({ comparison: comparison({ nothingToChange: true }) });
    await toProfile();
    await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
    await userEvent.click(cont());
    await screen.findByText(/already matches Web v2/);
    expect(
      screen.getByRole("heading", { name: "Nothing to change" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Preview changes" }),
    ).toBeNull();
    await userEvent.click(
      screen.getByRole("link", { name: "Back to projects" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent("/projects");
  });

  it("sends a folder with no harness back to the Folder step with Scaffold it instead", async () => {
    setup({
      comparison: comparison({
        rows: [rows[5] as Row],
      }),
    });
    await toProfile("/work/empty");
    await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
    await userEvent.click(cont());
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "has no Claude harness. Scaffold it instead.",
    );
    expect(screen.getByLabelText("Project folder")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Scaffold it instead" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent("/scaffold");
  });

  it("shows a core folder error on the Folder step and clears it on typing", async () => {
    setup({ compareError: "/work/x is not a folder" });
    await toProfile("/work/x");
    await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
    await userEvent.click(cont());
    expect(await screen.findByRole("alert")).toHaveTextContent("not a folder");
    await userEvent.type(screen.getByLabelText("Project folder"), "y");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps other compare errors on the Profile step", async () => {
    setup({ compareError: "Harneo is busy" });
    await toProfile();
    await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
    await userEvent.click(cont());
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Harneo is busy",
    );
    await userEvent.click(screen.getByRole("button", { name: /API/ }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("expands ~ against the home folder", async () => {
    const calls = setup();
    await toCompare("~/work/site");
    expect(calls.find((c) => c.cmd === "refit_compare")?.args).toMatchObject({
      target: "/home/me/work/site",
    });
  });

  it("keeps a planning error on the Compare step", async () => {
    setup({ planError: "site changed on disk" });
    await toCompare();
    await userEvent.click(
      screen.getByRole("button", { name: "Preview changes" }),
    );
    expect(await screen.findAllByRole("alert")).not.toHaveLength(0);
    expect(screen.getByText("site changed on disk")).toBeInTheDocument();
  });

  it("shows the apply failure and lets you go back", async () => {
    setup({ applyError: "Permission denied. Rolled back." });
    await toPreview();
    await userEvent.click(screen.getByRole("button", { name: /Apply refit/ }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Nothing was changed.");
    expect(alert).toHaveTextContent("Rolled back.");
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("table")).toBeInTheDocument();
  });

  it("restores the originals after a confirm", async () => {
    const calls = setup();
    await toPreview();
    await userEvent.click(screen.getByRole("button", { name: /Apply refit/ }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Restore originals" }),
    );
    // Cancel leaves everything as it was.
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(calls.some((c) => c.cmd === "backup_restore")).toBe(false);
    await userEvent.click(
      screen.getByRole("button", { name: "Restore originals" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(await screen.findAllByText(/Originals restored/)).not.toHaveLength(
      0,
    );
    expect(calls.find((c) => c.cmd === "backup_restore")?.args).toMatchObject({
      backupId: 5,
    });
    expect(
      screen.queryByRole("button", { name: "Restore originals" }),
    ).toBeNull();
  });

  it("shows a restore failure", async () => {
    setup({ restoreError: "backup folder missing" });
    await toPreview();
    await userEvent.click(screen.getByRole("button", { name: /Apply refit/ }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Restore originals" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Restore" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "backup folder missing",
    );
  });

  it("offers no restore when nothing was backed up", async () => {
    setup({ backupId: null });
    await toPreview();
    await userEvent.click(screen.getByRole("button", { name: /Apply refit/ }));
    await screen.findByText("Refitted site with Web v2");
    expect(
      screen.queryByRole("button", { name: "Restore originals" }),
    ).toBeNull();
  });

  it("offers Create a profile when there are none, and shows a list failure", async () => {
    setup({ profiles: [] });
    await toProfile();
    expect(await screen.findByText("No profiles yet")).toBeInTheDocument();
    expect(cont()).toBeDisabled();
    await userEvent.click(
      screen.getByRole("button", { name: "Create a profile" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent("/profiles/new");
  });

  it("shows a profile list failure", async () => {
    setup({ listError: "database locked" });
    await toProfile();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "database locked",
    );
  });

  it("goes Back from Profile to Folder keeping the path", async () => {
    setup();
    await toProfile();
    await userEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(screen.getByLabelText("Project folder")).toHaveValue("/work/site");
  });
});
