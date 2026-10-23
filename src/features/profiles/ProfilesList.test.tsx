import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ProfileImportPreview } from "../../bindings/ProfileImportPreview";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import { ProfilesScreen } from ".";

function summary(id: number, name: string, projectCount = 0): ProfileSummary {
  return {
    id: BigInt(id),
    name,
    description: "",
    version: 2,
    counts: [{ kind: "skill", count: 3 }],
    fileCount: 3,
    projectCount,
    updatedAt: 1_700_000_000n,
  };
}

const preview: ProfileImportPreview = {
  profile: {
    name: "Team",
    description: "",
    version: 1,
    spec: { items: [], settings: null, mcp: [], manualOnly: {} },
  },
  artifacts: [
    {
      id: "a",
      kind: "skill",
      name: "tdd",
      inLibrary: false,
      source: "team/harness",
      fileCount: 3,
    },
    {
      id: "b",
      kind: "agent",
      name: "reviewer",
      inLibrary: false,
      source: "team/harness",
      fileCount: 1,
    },
  ],
  nameClash: false,
};

function Where() {
  return <p data-testid="where">{useLocation().pathname}</p>;
}

function show() {
  render(
    <MemoryRouter initialEntries={["/profiles"]}>
      <Routes>
        <Route path="/profiles" element={<ProfilesScreen />} />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Profiles list states", () => {
  it("is busy while loading", async () => {
    mockIPC((cmd) => {
      if (cmd === "profiles_list") return new Promise(() => undefined);
      return null;
    });
    show();
    expect(
      await screen.findByRole("list", { name: "Loading profiles" }),
    ).toHaveAttribute("aria-busy", "true");
  });

  it("retries after a load error", async () => {
    const user = userEvent.setup();
    let calls = 0;
    mockIPC((cmd) => {
      if (cmd !== "profiles_list") return null;
      calls += 1;
      if (calls === 1) throw new Error("library is locked");
      return [summary(1, "Light")];
    });
    show();
    expect(await screen.findByText(/library is locked/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Light")).toBeVisible();
  });

  it("offers New profile in the empty state", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => (cmd === "profiles_list" ? [] : null));
    show();
    await screen.findByText("No profiles yet");
    await user.click(screen.getByRole("button", { name: "New profile" }));
    expect(await screen.findByTestId("where")).toHaveTextContent(
      "/profiles/new",
    );
  });
});

describe("Profile card", () => {
  it("links the project count to Projects", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) =>
      cmd === "profiles_list" ? [summary(1, "Light", 4)] : null,
    );
    show();
    await user.click(
      await screen.findByRole("link", { name: "Used in 4 projects" }),
    );
    expect(await screen.findByTestId("where")).toHaveTextContent("/projects");
  });

  it("opens the menu, closes it on Esc and on a click outside", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => (cmd === "profiles_list" ? [summary(1, "Light")] : null));
    show();
    const more = await screen.findByRole("button", {
      name: "More actions for Light",
    });
    await user.click(more);
    expect(screen.getByRole("menu")).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(more).toHaveFocus();
    await user.click(more);
    await user.click(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("renames a profile and reloads", async () => {
    const user = userEvent.setup();
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return cmd === "profiles_list" ? [summary(1, "Light")] : null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "More actions for Light" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Rename profile",
    });
    const submit = within(dialog).getByRole("button", { name: "Rename" });
    expect(submit).toBeDisabled();
    const field = within(dialog).getByLabelText("Name");
    await user.clear(field);
    await user.type(field, "Lighter");
    await user.click(submit);
    expect(await screen.findByText("Renamed Light to Lighter.")).toBeVisible();
    expect(calls.find((c) => c.cmd === "profile_rename")?.args).toEqual({
      id: 1,
      name: "Lighter",
    });
    expect(calls.filter((c) => c.cmd === "profiles_list")).toHaveLength(2);
  });

  it("shows why a rename was refused", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "profile_rename") throw new Error("name is taken");
      return cmd === "profiles_list" ? [summary(1, "Light")] : null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "More actions for Light" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText("Name"), "2");
    await user.click(within(dialog).getByRole("button", { name: "Rename" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "name is taken",
    );
  });

  it("deletes after a confirm", async () => {
    const user = userEvent.setup();
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return cmd === "profiles_list" ? [summary(1, "Light")] : null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "More actions for Light" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Delete Light?",
    });
    await user.click(
      within(dialog).getByRole("button", { name: "Delete profile" }),
    );
    expect(await screen.findByText("Deleted Light.")).toBeVisible();
    expect(calls.find((c) => c.cmd === "profile_delete")?.args).toEqual({
      id: 1,
    });
  });

  it("shows the projects that block a delete", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "profile_delete")
        throw new Error(
          "profile Light is used by api, web; remove it from them first",
        );
      return cmd === "profiles_list" ? [summary(1, "Light", 2)] : null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "More actions for Light" }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(
      within(dialog).getByRole("button", { name: "Delete profile" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      /used by api, web/,
    );
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

describe("Import preview provenance", () => {
  it("shows source and file count, the audit callout and the count on submit", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "profiles_list") return [];
      if (cmd === "plugin:dialog|open") return "/tmp/in.json";
      if (cmd === "profile_import_preview") return preview;
      return null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getAllByText("team/harness · 3 files")).toHaveLength(
      1,
    );
    expect(within(dialog).getByText("team/harness · 1 file")).toBeVisible();
    expect(
      within(dialog).getByText(
        /These 2 come from the file, not from a source you imported/,
      ),
    ).toBeVisible();
    expect(
      within(dialog).getByRole("button", {
        name: "Import profile and 2 artifacts",
      }),
    ).toBeVisible();
  });

  it("keeps the plain label when nothing is new", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "profiles_list") return [];
      if (cmd === "plugin:dialog|open") return "/tmp/in.json";
      if (cmd === "profile_import_preview")
        return {
          ...preview,
          artifacts: preview.artifacts.map((a) => ({ ...a, inLibrary: true })),
        };
      return null;
    });
    show();
    await user.click(
      await screen.findByRole("button", { name: "Import profile file" }),
    );
    const dialog = await screen.findByRole("dialog");
    expect(
      within(dialog).getByRole("button", { name: "Import profile" }),
    ).toBeVisible();
    expect(within(dialog).queryByText(/come from the file/)).toBeNull();
  });
});
