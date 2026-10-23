import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { AppRoutes } from "../../App";
import { openFolder } from "../../lib/ipc";
import { page } from "../../test/library";

const proj = (id: number, name: string) => ({
  id,
  path: `/work/${name}`,
  name,
  profileId: 1,
  profileName: "Web",
  profileVersion: 1,
  appliedAt: 1,
  fileCount: 2,
});

describe("projects in the app frame", () => {
  it("counts projects in the sidebar and lists them in the palette", async () => {
    mockIPC((cmd) => {
      if (cmd === "projects_list") return [proj(1, "shop"), proj(2, "blog")];
      if (cmd === "library_list") return page([]);
      if (cmd === "profiles_list") return [];
      return undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <AppRoutes />
      </MemoryRouter>,
    );
    const link = await screen.findByRole("link", { name: "Projects" });
    await waitFor(() => expect(link).toHaveTextContent("2"));
    const user = userEvent.setup();
    await user.keyboard("{Control>}k{/Control}");
    const list = await screen.findByRole("listbox", { name: "Results" });
    const group = await within(list).findByRole("group", { name: "Projects" });
    expect(group).toHaveTextContent("/work/shop");
    await user.click(within(group).getByRole("option", { name: /blog/ }));
    expect(
      await screen.findByRole("heading", { name: "Project" }),
    ).toBeInTheDocument();
  });

  it("shows no count and no group when the projects cannot load", async () => {
    mockIPC((cmd) => {
      if (cmd === "projects_list") throw new Error("down");
      if (cmd === "library_list") return page([]);
      if (cmd === "profiles_list") return [];
      return undefined;
    });
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <AppRoutes />
      </MemoryRouter>,
    );
    const link = await screen.findByRole("link", { name: "Projects" });
    expect(link).not.toHaveTextContent(/\d/);
    await userEvent.keyboard("{Control>}k{/Control}");
    const list = await screen.findByRole("listbox", { name: "Results" });
    expect(within(list).queryByRole("group", { name: "Projects" })).toBeNull();
  });
});

describe("openFolder", () => {
  it("asks the opener plugin to open the path", async () => {
    const calls: { cmd: string; args: unknown }[] = [];
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      return null;
    });
    await openFolder("/work/shop");
    expect(calls).toEqual([
      { cmd: "plugin:opener|open_path", args: { path: "/work/shop" } },
    ]);
  });
});

describe("import preset", () => {
  it("opens the folder source with the project path filled in", async () => {
    mockIPC(() => null);
    render(
      <MemoryRouter initialEntries={["/import?path=%2Fwork%2Fshop"]}>
        <AppRoutes />
      </MemoryRouter>,
    );
    expect(await screen.findByLabelText("Folder")).toHaveValue("/work/shop");
  });
});
