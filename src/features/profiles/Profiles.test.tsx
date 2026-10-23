import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ProfileImportPreview } from "../../bindings/ProfileImportPreview";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { Palette } from "../../app/Palette";
import { exportFileName, ProfileBuilder, ProfilesScreen } from ".";

function summary(id: number, name: string, over: Partial<ProfileSummary> = {}) {
  return {
    id: BigInt(id),
    name,
    description: `About ${name}`,
    version: 2,
    counts: [
      { kind: "skill", count: 3 },
      { kind: "agent", count: 1 },
    ],
    fileCount: 6,
    projectCount: 0,
    updatedAt: 1_700_000_000n,
    ...over,
  } satisfies ProfileSummary;
}

const preview: ProfileImportPreview = {
  profile: {
    name: "Team default",
    description: "",
    version: 3,
    spec: { items: [], settings: null, mcp: [], manualOnly: {} },
  },
  artifacts: [
    {
      id: "a",
      kind: "skill",
      name: "tdd",
      inLibrary: true,
      source: "addyosmani/agent-skills",
      fileCount: 2,
    },
    {
      id: "b",
      kind: "agent",
      name: "code-reviewer",
      inLibrary: false,
      source: "team/harness",
      fileCount: 1,
    },
  ],
  nameClash: false,
};

interface Opts {
  profiles?: ProfileSummary[];
  savePath?: string | null;
  openPath?: string | null;
  preview?: ProfileImportPreview;
  failList?: string;
  failClone?: string;
}

function Where() {
  return <p data-testid="where">{useLocation().pathname}</p>;
}

function setup(opts: Opts = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    switch (cmd) {
      case "profiles_list":
        if (opts.failList) throw new Error(opts.failList);
        return opts.profiles ?? [];
      // The dialogs are the backend's: it answers with the path it wrote, or null on cancel.
      case "profile_export":
        return opts.savePath === undefined ? "/tmp/x.json" : opts.savePath;
      case "profile_import_preview":
        return opts.openPath === null
          ? null
          : { token: "file-tok", ...(opts.preview ?? preview) };
      case "profile_import_commit":
        return { profileId: 9, name: "Team default", imported: ["b"] };
      case "profile_clone":
        if (opts.failClone) throw new Error(opts.failClone);
        return 7;
      case "profile_get":
        return {
          id: 7,
          name: "Light",
          description: "",
          version: 2,
          currentVersion: 2,
          spec: { items: ["a"], settings: null, mcp: [], manualOnly: {} },
          versions: [],
          createdAt: 1,
          updatedAt: 1,
        };
      case "profile_resolve":
        return {
          artifacts: [
            {
              id: "a",
              kind: "skill",
              name: "tdd",
              why: { type: "picked" },
              manualOnly: false,
              fileCount: 1,
            },
          ],
          missing: [],
          worstTrust: null,
        };
      case "library_list":
        return { items: [], total: 0, unfilteredTotal: 0 };
      case "profile_layout":
        return { paths: [], conflicts: [] };
      default:
        return null;
    }
  });
  render(
    <MemoryRouter initialEntries={["/profiles"]}>
      <Routes>
        <Route path="/profiles" element={<ProfilesScreen />} />
        <Route path="/profiles/:id" element={<ProfileBuilder />} />
      </Routes>
      <Where />
    </MemoryRouter>,
  );
  return calls;
}

describe("Profiles list", () => {
  it("shows cards with version, description, counts and usage", async () => {
    setup({
      profiles: [
        summary(1, "Light"),
        summary(2, "Heavy", { projectCount: 2, counts: [] }),
      ],
    });
    const light = (await screen.findByText("Light")).closest("li");
    expect(light).not.toBeNull();
    const card = within(light as HTMLElement);
    expect(card.getByText("v2")).toBeVisible();
    expect(card.getByText("About Light")).toBeVisible();
    expect(card.getByText("3 skills")).toBeVisible();
    expect(card.getByText("1 subagent")).toBeVisible();
    expect(card.getByText("Not used yet")).toBeVisible();
    expect(screen.getByText("Used in 2 projects")).toBeVisible();
  });

  it("shows the empty state", async () => {
    setup();
    expect(
      await screen.findByRole("heading", { name: "No profiles yet" }),
    ).toBeVisible();
  });

  it("shows a load error", async () => {
    setup({ failList: "library is locked" });
    expect(await screen.findByText(/library is locked/)).toBeVisible();
  });

  it("New profile goes to /profiles/new, also with N", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByText("No profiles yet");
    await user.click(screen.getByRole("button", { name: "New profile N" }));
    expect(screen.getByTestId("where")).toHaveTextContent("/profiles/new");
  });

  it("N starts a new profile", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByText("No profiles yet");
    await user.keyboard("n");
    expect(screen.getByTestId("where")).toHaveTextContent("/profiles/new");
  });

  it("clones under a new name and opens the copy", async () => {
    const user = userEvent.setup();
    const calls = setup({ profiles: [summary(1, "Light")] });
    await user.click(
      await screen.findByRole("button", { name: "Clone Light" }),
    );
    const dialog = screen.getByRole("dialog", { name: "Clone profile" });
    const name = within(dialog).getByLabelText("Name");
    expect(name).toHaveValue("Light copy");
    await user.clear(name);
    await user.type(name, "Light 2");
    await user.click(
      within(dialog).getByRole("button", { name: "Clone profile" }),
    );
    await waitFor(() =>
      expect(screen.getByTestId("where")).toHaveTextContent("/profiles/7"),
    );
    expect(calls.find((c) => c.cmd === "profile_clone")?.args).toEqual({
      id: 1,
      name: "Light 2",
    });
    expect(await screen.findByText("tdd")).toBeVisible();
  });

  it("closes the clone dialog with Esc", async () => {
    const user = userEvent.setup();
    setup({ profiles: [summary(1, "Light")] });
    await user.click(
      await screen.findByRole("button", { name: "Clone Light" }),
    );
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("keeps the clone dialog open with the error", async () => {
    const user = userEvent.setup();
    setup({ profiles: [summary(1, "Light")], failClone: "name is taken" });
    await user.click(
      await screen.findByRole("button", { name: "Clone Light" }),
    );
    await user.click(screen.getByRole("button", { name: "Clone profile" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("name is taken");
  });

  it("exports to the path the save dialog gave", async () => {
    const user = userEvent.setup();
    const calls = setup({
      profiles: [summary(1, "My Light")],
      savePath: "/home/me/my-light.harneo-profile.json",
    });
    await user.click(
      await screen.findByRole("button", { name: "Export My Light" }),
    );
    await waitFor(() =>
      expect(screen.getByText(/Exported My Light v2 to/)).toBeVisible(),
    );
    // No path goes to the backend: it opens the dialog and picks the path itself.
    expect(calls.find((c) => c.cmd === "profile_export")?.args).toEqual({
      id: 1,
      version: null,
      defaultName: "my-light.harneo-profile.json",
    });
    expect(calls.some((c) => c.cmd.startsWith("plugin:dialog"))).toBe(false);
  });

  it("does nothing when the save dialog is cancelled", async () => {
    const user = userEvent.setup();
    const calls = setup({ profiles: [summary(1, "Light")], savePath: null });
    await user.click(
      await screen.findByRole("button", { name: "Export Light" }),
    );
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "profile_export")).toBe(true),
    );
    expect(screen.queryByText(/Exported/)).toBeNull();
  });

  it("does nothing when the open dialog is cancelled", async () => {
    const user = userEvent.setup();
    const calls = setup({ openPath: null });
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === "profile_import_preview")).toBe(true),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(calls.some((c) => c.cmd === "profile_import_commit")).toBe(false);
  });

  it("previews a file, then commits it", async () => {
    const user = userEvent.setup();
    const calls = setup({ profiles: [summary(1, "Light")] });
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Import profile file",
    });
    expect(within(dialog).getByText(/1 of 2 artifacts/)).toBeVisible();
    expect(within(dialog).getByText("Already in your library")).toBeVisible();
    expect(within(dialog).getByText("tdd")).toBeVisible();
    expect(within(dialog).getByText("code-reviewer")).toBeVisible();
    await user.click(
      within(dialog).getByRole("button", {
        name: "Import profile and 1 artifact",
      }),
    );
    expect(await screen.findByText(/Imported Team default/)).toBeVisible();
    expect(calls.find((c) => c.cmd === "profile_import_commit")?.args).toEqual({
      token: "file-tok",
      name: null,
    });
    expect(calls.filter((c) => c.cmd === "profiles_list")).toHaveLength(2);
  });

  it("asks for another name on a name clash", async () => {
    const user = userEvent.setup();
    const calls = setup({ preview: { ...preview, nameClash: true } });
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("status")).toHaveTextContent(
      /already exists/,
    );
    expect(within(dialog).getByLabelText("Name")).toHaveValue(
      "Team default (imported)",
    );
    await user.click(
      within(dialog).getByRole("button", {
        name: "Import profile and 1 artifact",
      }),
    );
    await screen.findByText(/Imported Team default/);
    expect(calls.find((c) => c.cmd === "profile_import_commit")?.args).toEqual({
      token: "file-tok",
      name: "Team default (imported)",
    });
  });

  it("says when the file cannot be read", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "profiles_list") return [];
      throw new Error("not a Harneo profile file");
    });
    render(
      <MemoryRouter>
        <ProfilesScreen />
      </MemoryRouter>,
    );
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    expect(await screen.findByText(/not a Harneo profile file/)).toBeVisible();
  });
});

describe("exportFileName", () => {
  it("kebab-cases the name", () => {
    expect(exportFileName("My Light  profile!")).toBe(
      "my-light-profile.harneo-profile.json",
    );
    expect(exportFileName("???")).toBe("profile.harneo-profile.json");
  });
});

describe("command palette", () => {
  it("offers New profile and each profile as name and version", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => (cmd === "profiles_list" ? [summary(1, "Light")] : null));
    render(
      <MemoryRouter initialEntries={["/x"]}>
        <Palette onClose={() => undefined} onShortcuts={() => undefined} />
        <Where />
      </MemoryRouter>,
    );
    expect(
      await screen.findByRole("option", { name: "Light v2" }),
    ).toBeVisible();
    await user.type(screen.getByRole("combobox"), "new prof");
    await user.keyboard("{Enter}");
    expect(screen.getByTestId("where")).toHaveTextContent("/profiles/new");
  });
});
