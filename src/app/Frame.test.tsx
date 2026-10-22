import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AppRoutes } from "../App";
import { item, page } from "../test/library";

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockIPC((cmd) => {
    if (cmd === "library_list") return page([item("one")]);
    return cmd === "app_info" ? { version: "1.2.3", dataDir: "/d" } : undefined;
  });
});

const screens = [
  "Library",
  "Profiles",
  "Projects",
  "Security",
  "Import",
  "Scaffold project",
  "Refit project",
  "Cleanup",
  "Settings",
];

describe("routing", () => {
  it("redirects the root to the library", () => {
    renderAt("/");
    expect(
      screen.getByRole("heading", { level: 1, name: "Library" }),
    ).toBeInTheDocument();
  });

  it.each(screens)("sidebar entry %s routes to its screen", async (name) => {
    const user = userEvent.setup();
    renderAt("/library");
    const nav = screen.getByRole("navigation", { name: "Main" });
    await user.click(within(nav).getByRole("link", { name }));
    expect(screen.getByRole("heading", { level: 1, name })).toBeInTheDocument();
    expect(within(nav).getByRole("link", { name })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("shows the About panel on Settings", async () => {
    renderAt("/settings");
    expect(await screen.findByText("1.2.3")).toBeInTheDocument();
  });

  it("lists the empty Collections section", () => {
    renderAt("/library");
    expect(
      screen.getByRole("heading", { name: "Collections" }),
    ).toBeInTheDocument();
  });
});

describe("shortcuts dialog", () => {
  it("opens with ? and closes with Esc", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    await user.keyboard("?");
    const dialog = screen.getByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("Ctrl K")).toBeInTheDocument();
    expect(within(dialog).getByText("/")).toBeInTheDocument();
    expect(within(dialog).getByText("← ↑ → ↓")).toBeInTheDocument();
    expect(within(dialog).getByText("Home End")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens from the sidebar button and restores focus on close", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    const button = screen.getByRole("button", { name: /Keyboard shortcuts/ });
    await user.click(button);
    // Focus lands on the dialog's first control, so it is visible right away.
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(button).toHaveFocus();
  });
});

describe("palette", () => {
  it("opens with Ctrl K, filters, and navigates with Enter", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    await user.keyboard("{Control>}k{/Control}");
    expect(
      screen.getByRole("dialog", { name: "Search and commands" }),
    ).toBeInTheDocument();
    await user.keyboard("scaf{Enter}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 1, name: "Scaffold project" }),
    ).toBeInTheDocument();
  });

  it("moves with the arrow keys", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    await user.keyboard("{Control>}k{/Control}");
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    expect(
      screen.getByRole("heading", { level: 1, name: "Projects" }),
    ).toBeInTheDocument();
  });

  it("closes with Esc and navigates to an action by click", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    await user.click(screen.getByRole("button", { name: /Search/ }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Search/ }));
    await user.click(screen.getByRole("option", { name: /Cleanup/ }));
    expect(
      screen.getByRole("heading", { level: 1, name: "Cleanup" }),
    ).toBeInTheDocument();
  });

  it("ignores ? while typing in the palette input", async () => {
    const user = userEvent.setup();
    renderAt("/library");
    await user.keyboard("{Control>}k{/Control}");
    await user.keyboard("?");
    expect(
      screen.queryByRole("dialog", { name: "Keyboard shortcuts" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("?");
  });
});
