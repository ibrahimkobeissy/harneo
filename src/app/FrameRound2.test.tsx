import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AppRoutes } from "../App";
import { Tag } from "../ui";
import { AboutPanel } from "./AboutPanel";
import { modKey, searchShortcut } from "./platform";

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <AppRoutes />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockIPC((cmd) =>
    cmd === "app_info" ? { version: "1.2.3", dataDir: "/d" } : undefined,
  );
});

async function openPalette() {
  const user = userEvent.setup();
  renderAt("/library");
  await user.keyboard("{Control>}k{/Control}");
  return user;
}

describe("palette", () => {
  it("groups results under Places, Actions and Help, including Settings and shortcuts", async () => {
    await openPalette();
    const list = screen.getByRole("listbox", { name: "Results" });
    for (const g of ["Places", "Actions", "Help"]) {
      expect(within(list).getByRole("group", { name: g })).toBeInTheDocument();
    }
    expect(
      within(list).getByRole("option", { name: /Settings/ }),
    ).toBeInTheDocument();
    expect(
      within(list).getByRole("option", { name: /Keyboard shortcuts/ }),
    ).toBeInTheDocument();
  });

  it("wraps around with the arrow keys", async () => {
    const user = await openPalette();
    await user.keyboard("{ArrowUp}");
    const options = screen.getAllByRole("option");
    expect(options.at(-1)).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}");
    expect(options.at(0)).toHaveAttribute("aria-selected", "true");
  });

  it("follows the pointer", async () => {
    const user = await openPalette();
    const projects = screen.getByRole("option", { name: /Projects/ });
    await user.hover(projects);
    expect(projects).toHaveAttribute("aria-selected", "true");
  });

  it("opens the shortcuts list from the palette", async () => {
    const user = await openPalette();
    await user.type(screen.getByRole("combobox"), "keyboard");
    await user.keyboard("{Enter}");
    expect(
      screen.getByRole("dialog", { name: "Keyboard shortcuts" }),
    ).toBeInTheDocument();
  });

  it("echoes the query when nothing matches", async () => {
    const user = await openPalette();
    await user.type(screen.getByRole("combobox"), "zzz");
    expect(screen.getByRole("status")).toHaveTextContent(
      "No matches for “zzz”.",
    );
  });
});

describe("frame", () => {
  it("offers a skip link to the main content", () => {
    renderAt("/library");
    expect(
      screen.getByRole("link", { name: "Skip to content" }),
    ).toHaveAttribute("href", "#main");
  });

  it("marks only the Library item as current, not the brand", () => {
    renderAt("/library");
    expect(
      screen.getByRole("link", { name: /Harneo, go to Library/ }),
    ).not.toHaveAttribute("aria-current");
    expect(screen.getByRole("link", { name: "Library" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("defines each place on its placeholder screen", () => {
    renderAt("/profiles");
    expect(
      screen.getByText(/named selection of library artifacts/),
    ).toBeInTheDocument();
  });
});

describe("platform", () => {
  it("names the command key per OS", () => {
    expect(modKey("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe("⌘");
    expect(modKey("Mozilla/5.0 (X11; Linux x86_64)")).toBe("Ctrl");
    expect(searchShortcut("Mozilla/5.0 (X11; Linux x86_64)")).toBe("Ctrl K");
  });
});

describe("AboutPanel", () => {
  it("retries after a failure", async () => {
    let calls = 0;
    mockIPC((cmd) => {
      if (cmd !== "app_info") return undefined;
      calls += 1;
      if (calls === 1) throw "backend not ready";
      return { version: "1.2.3", dataDir: "/d" };
    });
    const user = userEvent.setup();
    render(<AboutPanel />);
    await user.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByText("1.2.3")).toBeInTheDocument();
  });
});

describe("Tag", () => {
  it("accepts the outline and ink families", () => {
    render(
      <>
        <Tag tint="plain">daily</Tag>
        <Tag tint="ink">docs</Tag>
      </>,
    );
    expect(screen.getByText("daily")).toHaveAttribute("data-tint", "plain");
    expect(screen.getByText("docs")).toHaveAttribute("data-tint", "ink");
  });
});
