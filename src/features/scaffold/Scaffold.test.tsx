import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { ScaffoldScreen } from ".";

const b64 = (s: string) => btoa(s);

function summary(id: number, name: string): ProfileSummary {
  return {
    id: BigInt(id),
    name,
    description: `About ${name}`,
    version: 2,
    counts: [
      { kind: "skill", count: 2 },
      { kind: "agent", count: 1 },
    ],
    fileCount: 3,
    projectCount: 0,
    updatedAt: 1n,
  };
}

const plan = {
  target: "/work/app",
  profileId: 1,
  profileName: "Web",
  profileVersion: 2,
  reason: "Scaffold Web v2",
  backupCount: 0,
  warnings: [],
  writes: [
    {
      path: "CLAUDE.md",
      action: "new",
      bytes: b64("# Hi\n"),
      diff: null,
      artifact: null,
    },
    {
      path: "HARNESS.md",
      action: "new",
      bytes: b64("# Harness\nline two\n"),
      diff: null,
      artifact: null,
    },
    {
      path: ".claude/skills/tdd/SKILL.md",
      action: "new",
      bytes: b64("tdd"),
      diff: null,
      artifact: "a",
    },
  ],
};

const pluginPlan = {
  ...plan,
  warnings: ["loose: not installable: no marketplace"],
  writes: plan.writes.map((w) =>
    w.path === "HARNESS.md"
      ? {
          ...w,
          bytes: b64(
            "# Harness\n\n## Plugins\n\nPlugins are not copied. `.claude/settings.json` enables them.\n",
          ),
        }
      : w,
  ),
};

interface Opts {
  plugins?: boolean;
  profiles?: ProfileSummary[];
  listError?: string;
  planError?: string;
  checkError?: string;
  applyError?: string;
  picked?: string | null;
  pickFails?: boolean;
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
        return opts.profiles ?? [summary(1, "Web")];
      case "plugin:dialog|open":
        if (opts.pickFails) throw new Error("no dialog");
        return opts.picked === undefined ? "/work/app" : opts.picked;
      case "home_claude_dir":
        return "/home/me/.claude";
      case "scaffold_check":
        if (opts.checkError) throw new Error(opts.checkError);
        return null;
      case "scaffold_plan":
        if (opts.planError) throw new Error(opts.planError);
        return opts.plugins ? pluginPlan : plan;
      case "install_apply":
        if (opts.applyError) throw new Error(opts.applyError);
        return {
          projectId: 7,
          written: 3,
          deleted: 0,
          backedUp: 0,
          backupId: null,
        };
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={["/scaffold"]}>
      <Routes>
        <Route path="/scaffold" element={<ScaffoldScreen />} />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

async function toProfile(path = "/work/app") {
  await userEvent.type(screen.getByLabelText("New project folder"), path);
  await userEvent.click(screen.getByRole("button", { name: "Continue" }));
}

async function toPreview() {
  await toProfile();
  await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
  await userEvent.click(screen.getByRole("button", { name: "Continue" }));
}

describe("Scaffold flow", () => {
  it("needs a folder before continuing, and Browse fills it in", async () => {
    setup();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    await waitFor(() =>
      expect(screen.getByLabelText("New project folder")).toHaveValue(
        "/work/app",
      ),
    );
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("keeps the path when the dialog is cancelled or fails", async () => {
    setup({ picked: null });
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    expect(screen.getByLabelText("New project folder")).toHaveValue("");
  });

  it("survives a failing dialog", async () => {
    setup({ pickFails: true });
    await userEvent.click(screen.getByRole("button", { name: /Browse/ }));
    expect(screen.getByLabelText("New project folder")).toHaveValue("");
  });

  it("walks Folder, Profile, Preview and Done", async () => {
    const calls = setup();
    await toPreview();
    expect(await screen.findByText(/Dry run/)).toBeInTheDocument();
    const call = calls.find((c) => c.cmd === "scaffold_plan");
    expect(call?.args).toMatchObject({ target: "/work/app", profileId: 1 });
    // Every file is new, HARNESS.md is open with its full content and line numbers.
    expect(screen.getAllByText("new")).toHaveLength(3);
    const code = screen.getByRole("region", { name: "Content of HARNESS.md" });
    expect(within(code).getByText("line two")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("treeitem", { name: /CLAUDE\.md/ }));
    expect(
      screen.getByRole("region", { name: "Content of CLAUDE.md" }),
    ).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole("button", { name: "Write 3 files" }),
    );
    expect(
      await screen.findByText("Scaffolded app with Web v2"),
    ).toBeInTheDocument();
    expect(screen.getByText(/3 files written, including/)).toBeInTheDocument();
    expect(calls.some((c) => c.cmd === "install_apply")).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Open project" }));
    expect(screen.getByTestId("where")).toHaveTextContent("/projects/7");
  });

  it("explains that plugins are enabled, not copied, and lists the ones that cannot be", async () => {
    setup({ plugins: true });
    await toPreview();
    expect(
      await screen.findByText("Plugins are not copied.", { selector: "b" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("enabledPlugins", { selector: "code" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("list", { name: "Plugins not enabled" }),
    ).toHaveTextContent("loose: not installable: no marketplace");
    await userEvent.click(
      screen.getByRole("button", { name: "Write 3 files" }),
    );
    expect(
      await screen.findByText(/Plugins were not copied/),
    ).toBeInTheDocument();
  });

  it("says nothing about plugins when the profile has none", async () => {
    setup();
    await toPreview();
    await screen.findByText(/Dry run/);
    expect(
      screen.queryByText("Plugins are not copied.", { selector: "b" }),
    ).toBeNull();
  });

  it("Scaffold another starts over", async () => {
    setup();
    await toPreview();
    await userEvent.click(
      await screen.findByRole("button", { name: "Write 3 files" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Scaffold another" }),
    );
    expect(screen.getByLabelText("New project folder")).toHaveValue("");
  });

  it("expands ~ against the home folder", async () => {
    const calls = setup();
    await toProfile("~/work/app");
    await userEvent.click(await screen.findByRole("button", { name: /Web/ }));
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Dry run/);
    expect(calls.find((c) => c.cmd === "scaffold_plan")?.args).toMatchObject({
      target: "/home/me/work/app",
    });
  });

  it("sends a folder with a harness back with Refit it instead", async () => {
    setup({
      planError: "/work/app has a harness (.claude). Refit it instead.",
    });
    await toPreview();
    expect(await screen.findByRole("alert")).toHaveTextContent("has a harness");
    expect(screen.getByLabelText("New project folder")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Refit it instead" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent(
      "/refit?path=%2Fwork%2Fapp",
    );
  });

  it("shows a folder error without the refit button, and clears it on typing", async () => {
    setup({ planError: "/work/app is not a folder" });
    await toPreview();
    expect(await screen.findByRole("alert")).toHaveTextContent("not a folder");
    expect(
      screen.queryByRole("button", { name: "Refit it instead" }),
    ).toBeNull();
    await userEvent.type(screen.getByLabelText("New project folder"), "x");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps other planning errors on the Profile step", async () => {
    setup({ planError: "Harneo is busy" });
    await toPreview();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Harneo is busy",
    );
    expect(screen.getByRole("button", { name: /Web/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Web/ }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows the write failure and lets you go back", async () => {
    setup({
      applyError: "Permission denied. Rolled back: nothing was changed.",
    });
    await toPreview();
    await userEvent.click(
      await screen.findByRole("button", { name: "Write 3 files" }),
    );
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Nothing was written.");
    expect(alert).toHaveTextContent("Rolled back: nothing was changed.");
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("button", { name: /Web/ })).toBeInTheDocument();
  });

  it("offers Create a profile when there are none", async () => {
    setup({ profiles: [] });
    await toProfile();
    expect(await screen.findByText("No profiles yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();
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
    expect(screen.getByLabelText("New project folder")).toHaveValue(
      "/work/app",
    );
  });

  it("checks the folder on step 1 and keeps the user there with focus on the field", async () => {
    const calls = setup({
      checkError: "/work/app has a harness (.claude). Refit it instead.",
    });
    await toProfile();
    expect(await screen.findByRole("alert")).toHaveTextContent("has a harness");
    expect(screen.getByLabelText("New project folder")).toHaveFocus();
    expect(screen.queryByRole("button", { name: /Web/ })).toBeNull();
    expect(calls.some((c) => c.cmd === "profiles_list")).toBe(true);
    expect(calls.find((c) => c.cmd === "scaffold_check")?.args).toEqual({
      target: "/work/app",
    });
    expect(calls.some((c) => c.cmd === "scaffold_plan")).toBe(false);
    await userEvent.click(
      screen.getByRole("button", { name: "Refit it instead" }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent(
      "/refit?path=%2Fwork%2Fapp",
    );
  });

  it("refuses a folder that is not writable on step 1 and lets you retry", async () => {
    setup({ checkError: "/work/app is not writable" });
    await toProfile();
    expect(await screen.findByRole("alert")).toHaveTextContent("not writable");
    expect(
      screen.queryByRole("button", { name: "Refit it instead" }),
    ).toBeNull();
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("shows profile cards with counts by kind", async () => {
    setup();
    await toProfile();
    const card = await screen.findByRole("button", { name: /Web/ });
    expect(card).toHaveTextContent("2 skills");
    expect(card).toHaveTextContent("1 subagent");
    expect(card).toHaveTextContent("3 files");
  });

  it("says no harness files exist, not that nothing exists", async () => {
    setup();
    await toPreview();
    expect(
      await screen.findByText(/No harness files exist there yet/),
    ).toBeInTheDocument();
  });
});
