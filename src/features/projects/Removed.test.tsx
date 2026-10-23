import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it } from "vitest";
import type { Backup } from "../../bindings/Backup";
import type { Project } from "../../bindings/Project";
import { ProjectScreen, ProjectsScreen } from ".";

afterEach(() => clearMocks());

const gone: Project = {
  id: 7n,
  path: "/work/old",
  name: "old",
  profileId: 0n,
  profileName: "Web",
  profileVersion: 2,
  appliedAt: 100n,
  fileCount: 0,
  removedAt: 200n,
};

const set: Backup = {
  id: 3n,
  projectId: 7n,
  createdAt: 200n,
  reason: "Remove Web v2",
  fileCount: 1,
  folder: "backups/7/2026-10-23T10-00-00Z",
};

describe("removed projects", () => {
  it("are listed under the live projects and open on their backups", async () => {
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      switch (cmd) {
        case "projects_list":
          return [];
        case "projects_removed":
          return [gone];
        case "project_get":
          return gone;
        case "project_backups":
          return [set];
        case "backup_restore":
          return {
            projectId: 7,
            written: 1,
            deleted: 0,
            backedUp: 0,
            backupId: null,
            backupFolder: null,
          };
        default:
          return [];
      }
    });
    render(
      <MemoryRouter initialEntries={["/projects"]}>
        <Routes>
          <Route path="/projects" element={<ProjectsScreen />} />
          <Route path="/projects/:id" element={<ProjectScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    const user = userEvent.setup();
    const list = await screen.findByRole("list", { name: "Removed projects" });
    await user.click(within(list).getByRole("link", { name: "old" }));
    expect(await screen.findByText(/harness was removed/)).toBeInTheDocument();
    expect(calls).not.toContain("drift_check");
    expect(calls).not.toContain("project_files");
    await user.click(await screen.findByRole("button", { name: /Restore/ }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: /Restore/ }));
    await waitFor(() => expect(calls).toContain("backup_restore"));
  });

  it("stay out of the page when none were removed or the list fails", async () => {
    mockIPC((cmd) => {
      if (cmd === "projects_removed") throw new Error("down");
      return [];
    });
    render(
      <MemoryRouter>
        <ProjectsScreen />
      </MemoryRouter>,
    );
    await screen.findByText("No projects yet");
    expect(screen.queryByText("Removed projects")).toBeNull();
  });
});
