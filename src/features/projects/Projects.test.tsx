import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it, vi } from "vitest";
import type { Backup } from "../../bindings/Backup";
import type { Plan } from "../../bindings/Plan";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import type { Project } from "../../bindings/Project";
import { ProjectScreen, ProjectsScreen } from ".";
import { Toast } from "./Toast";
import { ago, dateOf, statusOf, toBase64 } from "./format";

const NOW = Math.floor(Date.now() / 1000);

function project(over: Partial<Project> = {}): Project {
  return {
    id: 1n,
    path: "/work/shop",
    name: "shop",
    profileId: 10n,
    profileName: "Web",
    profileVersion: 1,
    appliedAt: BigInt(NOW - 3 * 3600),
    fileCount: 3,
    ...over,
  };
}

function profile(id: number, name: string, version: number): ProfileSummary {
  return {
    id: BigInt(id),
    name,
    description: `About ${name}`,
    version,
    counts: [],
    fileCount: 3,
    projectCount: 1,
    updatedAt: 1n,
  };
}

function backup(id: number, reason: string): Backup {
  return {
    id: BigInt(id),
    projectId: 1n,
    createdAt: BigInt(NOW - 86400),
    reason,
    fileCount: 2,
    folder: `backups/shop/2026-10-0${id}`,
  };
}

const b64 = (s: string) => btoa(s);

function plan(over: Partial<Plan> = {}): Plan {
  return {
    target: "/work/shop",
    profileId: 11n,
    profileName: "Api",
    profileVersion: 3,
    reason: "Switch to Api v3",
    backupCount: 1,
    kind: "switch",
    editedInProject: [".claude/agents/old.md"],
    restored: [],
    writes: [
      {
        path: ".claude/skills/new/SKILL.md",
        action: "new",
        bytes: b64("n"),
        diff: null,
        artifact: "a",
        executable: false,
      },
      {
        path: ".claude/agents/old.md",
        action: "delete",
        bytes: null,
        diff: null,
        artifact: "b",
        executable: false,
      },
      {
        path: "CLAUDE.md",
        action: "replace",
        bytes: b64("c"),
        diff: null,
        artifact: null,
        executable: false,
      },
      {
        path: "HARNESS.md",
        action: "keep",
        bytes: null,
        diff: null,
        artifact: null,
        executable: false,
      },
    ],
    ...over,
  };
}

interface Opts {
  projects?: Project[];
  profiles?: ProfileSummary[];
  backups?: Backup[];
  listError?: string;
  getError?: string;
  contentError?: string;
  profilesError?: boolean;
  planError?: string;
  applyError?: string;
  restoreError?: string;
  openError?: string;
  removalPlan?: Plan;
  truncated?: boolean;
}

function Where() {
  const l = useLocation();
  return (
    <p data-testid="where">
      {l.pathname + l.search}|{JSON.stringify(l.state)}
    </p>
  );
}

function setup(path: string, opts: Opts = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "projects_list":
        if (opts.listError) throw new Error(opts.listError);
        return opts.projects ?? [project()];
      case "project_get":
        if (opts.getError) throw new Error(opts.getError);
        return (opts.projects ?? [project()])[0];
      case "profiles_list":
        if (opts.profilesError) throw new Error("no profiles");
        return opts.profiles ?? [profile(10, "Web", 1), profile(11, "Api", 3)];
      case "project_files":
        return [
          { path: "CLAUDE.md", digest: "d", artifactId: null },
          { path: "HARNESS.md", digest: "e", artifactId: null },
          { path: ".claude/skills/tdd/SKILL.md", digest: "f", artifactId: "a" },
        ];
      case "project_file_content": {
        if (opts.contentError) throw new Error(opts.contentError);
        const p = (args as { path: string }).path;
        return {
          path: p,
          content: `# ${p}\nline two ✓\n`,
          truncated: opts.truncated ?? false,
        };
      }
      case "project_backups":
        return (
          opts.backups ?? [backup(2, "Switch to Web v1"), backup(1, "Refit")]
        );
      case "app_info":
        return { version: "1", dataDir: "/data/harneo" };
      case "switch_plan":
        if (opts.planError) throw new Error(opts.planError);
        return plan();
      case "removal_plan":
        if (opts.planError) throw new Error(opts.planError);
        return (
          opts.removalPlan ??
          plan({
            kind: "removal",
            editedInProject: [],
            restored: ["CLAUDE.md"],
            writes: [
              {
                path: "CLAUDE.md",
                action: "replace",
                bytes: b64("o"),
                diff: null,
                artifact: null,
                executable: false,
              },
              {
                path: "HARNESS.md",
                action: "delete",
                bytes: null,
                diff: null,
                artifact: null,
                executable: false,
              },
              {
                path: ".claude/x.md",
                action: "delete",
                bytes: null,
                diff: null,
                artifact: null,
                executable: false,
              },
            ],
          })
        );
      case "install_apply":
        if (opts.applyError) throw new Error(opts.applyError);
        return {
          projectId: 1,
          written: 2,
          deleted: 1,
          backedUp: 2,
          backupId: 5,
          backupFolder: "backups/shop/2026-10-23",
        };
      case "backup_restore":
        if (opts.restoreError) throw new Error(opts.restoreError);
        return {
          projectId: 1,
          written: 2,
          deleted: 0,
          backedUp: 2,
          backupId: 9,
          backupFolder: null,
        };
      case "backup_delete":
        if (opts.restoreError) throw new Error(opts.restoreError);
        return null;
      case "plugin:opener|open_path":
        if (opts.openError) throw new Error(opts.openError);
        return null;
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/projects" element={<ProjectsScreen />} />
        <Route path="/projects/:id" element={<ProjectScreen />} />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

describe("format helpers", () => {
  it("words a time as relative, then as a date", () => {
    const now = 1_000_000_000_000;
    const s = BigInt(now / 1000);
    expect(ago(s, now)).toBe("just now");
    expect(ago(s - 120n, now)).toBe("2 minutes ago");
    expect(ago(s - 3600n, now)).toBe("1 hour ago");
    expect(ago(s - 86400n * 3n, now)).toBe("3 days ago");
    expect(ago(s - 86400n * 30n, now)).toBe(dateOf(s - 86400n * 30n));
    expect(ago(s + 500n, now)).toBe("just now");
  });
  it("flags only a newer profile version", () => {
    expect(statusOf(project(), 2)).toEqual({ kind: "available", version: 2 });
    expect(statusOf(project(), 1)).toEqual({ kind: "sync" });
    expect(statusOf(project(), undefined)).toEqual({ kind: "sync" });
  });
  it("encodes UTF-8 text as base64", () => {
    expect(atob(toBase64("é"))).toBe("Ã©");
  });
});

describe("Projects list", () => {
  it("shows a card per project with profile, status, applied time and file count", async () => {
    setup("/projects", {
      projects: [
        project(),
        project({
          id: 2n,
          name: "blog",
          path: "/work/blog",
          profileId: 11n,
          profileName: "Api",
          profileVersion: 3,
          fileCount: 1,
        }),
      ],
    });
    const list = await screen.findByRole("list", { name: "Projects" });
    const cards = within(list).getAllByRole("link");
    expect(cards).toHaveLength(2);
    expect(cards[0]).toHaveTextContent("shop");
    expect(cards[0]).toHaveTextContent("/work/shop");
    expect(cards[0]).toHaveTextContent("Web v1");
    expect(cards[0]).toHaveTextContent("In sync");
  });

  it("says In sync when the profile has not moved on", async () => {
    setup("/projects", {
      projects: [project({ profileVersion: 1 })],
      profiles: [profile(10, "Web", 1)],
    });
    expect(await screen.findByText("In sync")).toBeInTheDocument();
    expect(screen.getByText("Applied 3 hours ago")).toBeInTheDocument();
    expect(screen.getByText("3 files")).toBeInTheDocument();
  });

  it("shows a newer profile version as a pill", async () => {
    setup("/projects", { profiles: [profile(10, "Web", 4)] });
    expect(await screen.findByText("Profile v4 available")).toBeInTheDocument();
  });

  it("keeps working when the profiles cannot load", async () => {
    setup("/projects", { profilesError: true });
    expect(await screen.findByText("In sync")).toBeInTheDocument();
  });

  it("has an empty state with both ways in", async () => {
    setup("/projects", { projects: [] });
    expect(await screen.findByText("No projects yet")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Scaffold new/ }));
    expect(screen.getByTestId("where")).toHaveTextContent("/scaffold");
  });

  it("goes to refit from the header", async () => {
    setup("/projects", { projects: [] });
    await userEvent.click(
      await screen.findByRole("button", { name: /Refit existing/ }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent("/refit");
  });

  it("shows a load failure", async () => {
    setup("/projects", { listError: "db locked" });
    expect(await screen.findByRole("alert")).toHaveTextContent("db locked");
  });

  it("opens a project from its card", async () => {
    setup("/projects");
    await userEvent.click(await screen.findByRole("link", { name: /shop/ }));
    expect(
      await screen.findByRole("heading", { name: "shop" }),
    ).toBeInTheDocument();
  });
});

describe("Project detail", () => {
  it("shows the header, stat panels and the installed files with content", async () => {
    setup("/projects/1", { profiles: [profile(10, "Web", 1)] });
    expect(
      await screen.findByRole("heading", { name: "shop" }),
    ).toBeInTheDocument();
    expect(screen.getByText("/work/shop")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Web" })).toHaveAttribute(
      "href",
      "/profiles/10",
    );
    expect(screen.getByText("3 hours ago")).toBeInTheDocument();
    expect(screen.queryByText(/Upgrade to/)).not.toBeInTheDocument();
    const view = await screen.findByRole("region", {
      name: "Content of HARNESS.md",
    });
    expect(view).toHaveTextContent("# HARNESS.md");
    await userEvent.click(screen.getByRole("button", { name: "CLAUDE.md" }));
    expect(
      await screen.findByRole("region", { name: "Content of CLAUDE.md" }),
    ).toHaveTextContent("line two ✓");
  });

  it("notes a truncated file and a file that cannot be read", async () => {
    setup("/projects/1", { truncated: true });
    expect(
      await screen.findByText(/only the start is shown/),
    ).toBeInTheDocument();
  });

  it("shows a read failure in the viewer", async () => {
    setup("/projects/1", { contentError: "gone" });
    expect(
      await screen.findByText(/Could not read HARNESS.md/),
    ).toHaveTextContent("gone");
  });

  it("says when Harneo wrote no files", async () => {
    mockIPC((cmd) => {
      if (cmd === "project_get") return project();
      if (cmd === "project_files" || cmd === "project_backups") return [];
      if (cmd === "drift_check") return null;
      return [];
    });
    render(
      <MemoryRouter initialEntries={["/projects/1"]}>
        <Routes>
          <Route path="/projects/:id" element={<ProjectScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(
      await screen.findByText(/has not written any files/),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("tab", { name: /Backups/ }));
    expect(screen.getByText("No backups yet.")).toBeInTheDocument();
  });

  it("is not found for a bad id or a missing project", async () => {
    setup("/projects/abc");
    expect(await screen.findByText("Project not found")).toBeInTheDocument();
  });

  it("shows why a project could not load", async () => {
    setup("/projects/1", { getError: "no such project" });
    expect(await screen.findByText(/no such project/)).toBeInTheDocument();
  });

  it("opens the folder, and reports when it cannot", async () => {
    const calls = setup("/projects/1");
    await userEvent.click(
      await screen.findByRole("button", { name: /^Open$/ }),
    );
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "plugin:opener|open_path")).toBe(true),
    );
    expect(
      calls.find((c) => c.cmd === "plugin:opener|open_path")?.args,
    ).toMatchObject({
      path: "/work/shop",
    });
  });

  it("toasts when opening fails, and the toast can be dismissed", async () => {
    setup("/projects/1", { openError: "no file manager" });
    await userEvent.click(
      await screen.findByRole("button", { name: /^Open$/ }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "no file manager",
    );
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("sends the folder to the import flow", async () => {
    setup("/projects/1");
    await userEvent.click(
      await screen.findByRole("button", { name: /Import from this project/ }),
    );
    expect(screen.getByTestId("where")).toHaveTextContent(
      "/import?path=%2Fwork%2Fshop",
    );
  });
});

describe("Backups", () => {
  async function toBackups(opts: Opts = {}) {
    const calls = setup("/projects/1", opts);
    await userEvent.click(await screen.findByRole("tab", { name: /Backups/ }));
    return calls;
  }

  it("lists backups with reason, count and where they live", async () => {
    await toBackups();
    const list = screen.getByRole("list", { name: "Backups" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(list).toHaveTextContent("Switch to Web v1");
    expect(list).toHaveTextContent("2 files");
    expect(screen.getByText("/data/harneo/backups/shop/")).toBeInTheDocument();
  });

  it("restores after a confirm", async () => {
    const calls = await toBackups();
    await userEvent.click(
      screen.getAllByRole("button", { name: /Restore/ })[0] as HTMLElement,
    );
    const dialog = screen.getByRole("dialog", { name: "Restore this backup?" });
    expect(dialog).toHaveTextContent("backed up first");
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Restore" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Restored 2 files",
    );
    expect(calls.find((c) => c.cmd === "backup_restore")?.args).toEqual({
      backupId: 2,
    });
  });

  it("deletes after a confirm", async () => {
    const calls = await toBackups();
    await userEvent.click(
      screen.getByRole("button", { name: "Delete backup Refit" }),
    );
    const dialog = screen.getByRole("dialog", { name: "Delete this backup?" });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Delete backup" }),
    );
    await waitFor(() =>
      expect(calls.find((c) => c.cmd === "backup_delete")?.args).toEqual({
        backupId: 1,
      }),
    );
    expect(await screen.findByText("Deleted the backup.")).toBeInTheDocument();
  });

  it("keeps the dialog and shows the error when a restore fails, and Cancel closes it", async () => {
    await toBackups({ restoreError: "disk full" });
    await userEvent.click(
      screen.getAllByRole("button", { name: /Restore/ })[0] as HTMLElement,
    );
    const dialog = screen.getByRole("dialog");
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Restore" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "disk full",
    );
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Cancel" }),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes the confirm with Escape", async () => {
    await toBackups();
    await userEvent.click(
      screen.getAllByRole("button", { name: /Restore/ })[0] as HTMLElement,
    );
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("falls back to a plain backups folder without rows or a data dir", async () => {
    await toBackups({ backups: [] });
    expect(screen.getByText("backups/", { exact: false })).toBeInTheDocument();
  });
});

describe("Switch / Upgrade modal", () => {
  async function upgradeSetup(opts: Opts = {}) {
    const calls = setup("/projects/1", {
      profiles: [profile(10, "Web", 3), profile(11, "Api", 3)],
      ...opts,
    });
    await userEvent.click(
      await screen.findByRole("button", { name: "Upgrade to v3" }),
    );
    return calls;
  }

  it("upgrades: groups the plan, flags edited files, applies", async () => {
    const calls = await upgradeSetup();
    const dialog = await screen.findByRole("dialog", { name: "Upgrade shop" });
    expect(within(dialog).queryByRole("radio")).not.toBeInTheDocument();
    expect(
      await within(dialog).findByText(".claude/skills/new/SKILL.md"),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("list", { name: "Add" }),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("list", {
        name: "Edited in project, backed up first",
      }),
    ).toHaveTextContent(".claude/agents/old.md");
    expect(
      within(dialog).getByRole("list", { name: "Update" }),
    ).toHaveTextContent("CLAUDE.md");
    expect(dialog).toHaveTextContent(
      "1 file backed up before anything changes, to /data/harneo/backups/",
    );
    expect(within(dialog).queryByText("HARNESS.md")).not.toBeInTheDocument();
    expect(dialog).toHaveTextContent("One file was edited in the project");
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Apply difference" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "shop now runs Api v3",
    );
    expect(calls.find((c) => c.cmd === "switch_plan")?.args).toEqual({
      projectId: 1,
      profileId: 10,
      version: null,
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows a plan failure and a failed apply, and stays open", async () => {
    await upgradeSetup({ planError: "profile gone" });
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "profile gone",
    );
    expect(
      within(dialog).getByRole("button", { name: "Apply difference" }),
    ).toBeDisabled();
  });

  it("reports a failed apply", async () => {
    await upgradeSetup({ applyError: "write failed" });
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText(".claude/skills/new/SKILL.md");
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Apply difference" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "write failed",
    );
    expect(
      within(dialog).getByRole("button", { name: "Apply difference" }),
    ).toBeEnabled();
  });

  it("says when only the version changes", async () => {
    mockIPC((cmd) => {
      if (cmd === "project_get") return project();
      if (cmd === "profiles_list") return [profile(10, "Web", 2)];
      if (cmd === "switch_plan")
        return plan({ writes: [], editedInProject: [] });
      if (cmd === "drift_check") return null;
      return [];
    });
    render(
      <MemoryRouter initialEntries={["/projects/1"]}>
        <Routes>
          <Route path="/projects/:id" element={<ProjectScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Upgrade to v2" }),
    );
    expect(
      await screen.findByText("Same files; only the version changes."),
    ).toBeInTheDocument();
  });

  it("switches: radio list of the other profiles, re-plans on pick", async () => {
    const calls = setup("/projects/1", {
      profiles: [
        profile(10, "Web", 1),
        profile(11, "Api", 3),
        profile(12, "Cli", 2),
      ],
    });
    await userEvent.click(
      await screen.findByRole("button", { name: /Switch profile/ }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Switch shop to another profile",
    });
    const radios = within(dialog).getAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[0]).toBeChecked();
    await within(dialog).findByText(".claude/skills/new/SKILL.md");
    await userEvent.click(radios[1] as HTMLElement);
    await waitFor(() =>
      expect(
        calls
          .filter((c) => c.cmd === "switch_plan")
          .map((c) => (c.args as { profileId: number }).profileId),
      ).toEqual([11, 12]),
    );
  });

  it("has nothing to switch to with a single profile", async () => {
    setup("/projects/1", { profiles: [profile(10, "Web", 1)] });
    await userEvent.click(
      await screen.findByRole("button", { name: /Switch profile/ }),
    );
    expect(await screen.findByText(/no other profile/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

async function openRemove() {
  await userEvent.click(
    await screen.findByRole("button", { name: "More actions" }),
  );
  await userEvent.click(
    await screen.findByRole("menuitem", { name: "Remove the harness" }),
  );
}

describe("Clean removal", () => {
  async function open(opts: Opts = {}) {
    const calls = setup("/projects/1", opts);
    await openRemove();
    return calls;
  }

  it("counts files from the removal plan and returns to the list with a toast", async () => {
    const calls = await open();
    const dialog = await screen.findByRole("dialog", {
      name: "Remove the harness from shop?",
    });
    expect(
      await within(dialog).findByText(
        /Removes the 2 files Harneo wrote and restores the 1 original it backed up/,
      ),
    ).toBeInTheDocument();
    expect(dialog).toHaveTextContent("CLAUDE.local.md");
    expect(dialog).toHaveTextContent("The project leaves the registry.");
    expect(
      within(dialog).getByRole("list", {
        name: "Restored: your original comes back",
      }),
    ).toHaveTextContent("CLAUDE.md");
    expect(
      within(dialog).getByRole("list", {
        name: "Deleted: Harneo created these",
      }),
    ).toHaveTextContent("HARNESS.md");
    expect(within(dialog).queryByText("Update")).toBeNull();
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Remove and restore originals",
      }),
    );
    const toast = await screen.findByRole("status");
    expect(toast).toHaveTextContent(
      "Removed the harness from shop; 1 original restored.",
    );
    expect(toast).toHaveTextContent("/data/harneo/backups/shop/2026-10-23");
    expect(calls.some((c) => c.cmd === "install_apply")).toBe(true);
  });

  it("keeps the removal toast until dismissed, with copy and open", async () => {
    const calls = await open();
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("CLAUDE.md");
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Remove and restore originals",
      }),
    );
    const toast = await screen.findByRole("status");
    await userEvent.click(
      within(toast).getByRole("button", { name: "Open backup folder" }),
    );
    await waitFor(() =>
      expect(
        calls.find((c) => c.cmd === "plugin:opener|open_path")?.args,
      ).toMatchObject({ path: "/data/harneo/backups/shop/2026-10-23" }),
    );
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    await userEvent.click(
      within(toast).getByRole("button", { name: "Copy path" }),
    );
    expect(writeText).toHaveBeenCalledWith(
      "/data/harneo/backups/shop/2026-10-23",
    );
    expect(
      await within(toast).findByRole("button", { name: "Copied" }),
    ).toBeInTheDocument();
    vi.unstubAllGlobals();
    await userEvent.click(
      within(toast).getByRole("button", { name: "Dismiss" }),
    );
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows the toast on the projects list", async () => {
    mockIPC((cmd) =>
      cmd === "projects_list" || cmd === "profiles_list" ? [] : null,
    );
    render(
      <MemoryRouter
        initialEntries={[
          { pathname: "/projects", state: { toast: "Removed it." } },
        ]}
      >
        <Routes>
          <Route path="/projects" element={<ProjectsScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    expect(await screen.findByRole("status", { name: "" })).toHaveTextContent(
      "Removed it.",
    );
  });

  it("flags edited files, and fails without removing", async () => {
    await open({
      applyError: "locked",
      removalPlan: plan({
        kind: "removal",
        editedInProject: ["a.md", "b.md"],
        restored: [],
        writes: [
          {
            path: "a.md",
            action: "delete",
            bytes: null,
            diff: null,
            artifact: null,
            executable: false,
          },
        ],
      }),
    });
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByText(/2 files were edited/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole("list", {
        name: "Edited in project, backed up first",
      }),
    ).toHaveTextContent("a.md");
    await userEvent.click(
      within(dialog).getByRole("button", {
        name: "Remove and restore originals",
      }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "locked",
    );
  });

  it("reports a plan failure and keeps the action disabled", async () => {
    await open({ planError: "no manifest" });
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "no manifest",
    );
    expect(
      within(dialog).getByRole("button", {
        name: "Remove and restore originals",
      }),
    ).toBeDisabled();
  });
});

describe("Toast", () => {
  it("offers copy and open for a folder and survives a failing clipboard", async () => {
    const open = vi.fn();
    render(
      <Toast
        notice={{ text: "Backed up in ", tone: "ok", folder: "/data/b" }}
        onDismiss={() => undefined}
        onOpenFolder={open}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Copy path" }));
    expect(screen.getByRole("button", { name: "Copy path" })).toBeVisible();
    await userEvent.click(
      screen.getByRole("button", { name: "Open backup folder" }),
    );
    expect(open).toHaveBeenCalledWith("/data/b");
  });

  it("has no open button without a handler, and a plain success leaves by itself", async () => {
    vi.useFakeTimers();
    const dismiss = vi.fn();
    const { rerender } = render(
      <Toast
        notice={{ text: "x", tone: "ok", folder: "/data/b" }}
        onDismiss={dismiss}
      />,
    );
    expect(
      screen.queryByRole("button", { name: "Open backup folder" }),
    ).toBeNull();
    vi.advanceTimersByTime(60000);
    expect(dismiss).not.toHaveBeenCalled();
    rerender(<Toast notice={{ text: "y", tone: "ok" }} onDismiss={dismiss} />);
    vi.advanceTimersByTime(9000);
    expect(dismiss).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2000);
    expect(dismiss).toHaveBeenCalled();
    vi.useRealTimers();
  });
});

describe("Overflow menu", () => {
  it("closes on Esc and on a click elsewhere, and toggles from its button", async () => {
    setup("/projects/1");
    const more = await screen.findByRole("button", { name: "More actions" });
    await userEvent.click(more);
    expect(
      screen.getByRole("menuitem", { name: "Remove the harness" }),
    ).toBeVisible();
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    await userEvent.click(more);
    await userEvent.click(more);
    expect(screen.queryByRole("menu")).toBeNull();
    await userEvent.click(more);
    await userEvent.click(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
    await userEvent.click(more);
    await userEvent.click(screen.getByRole("menu"));
    expect(screen.getByRole("menu")).toBeVisible();
  });
});
