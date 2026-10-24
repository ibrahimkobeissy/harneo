import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HashRouter, MemoryRouter, useLocation } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AppRoutes } from "../App";
import { item, page } from "../test/library";
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
  mockIPC((cmd, args) => {
    // Artifact "one" matches only an unfiltered list, so a palette search for "zzz" finds nothing.
    if (cmd === "library_list") {
      const text = (args as { query: { filter: { text: string | null } } })
        .query.filter.text;
      return page(text ? [] : [item("one")]);
    }
    if (cmd === "profiles_list") return [];
    return cmd === "app_info" ? { version: "1.2.3", dataDir: "/d" } : undefined;
  });
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

  it("offers a Library search when nothing matches, and Enter runs it", async () => {
    const user = await openPalette();
    await user.type(screen.getByRole("combobox"), "zzz");
    expect(
      screen.getByRole("button", { name: "Search the Library for it" }),
    ).toBeInTheDocument();
    await user.keyboard("{Enter}");
    expect(
      screen.queryByRole("dialog", { name: "Search and commands" }),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole("searchbox")).toHaveValue("zzz"),
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

  it("skip link focuses the main content and keeps the current route", async () => {
    const user = userEvent.setup();
    window.location.hash = "#/profiles";
    render(
      <HashRouter>
        <AppRoutes />
      </HashRouter>,
    );
    await user.click(screen.getByRole("link", { name: "Skip to content" }));
    expect(screen.getByRole("main")).toHaveFocus();
    expect(window.location.hash).toBe("#/profiles");
    expect(screen.getByRole("link", { name: "Profiles" })).toHaveAttribute(
      "aria-current",
      "page",
    );
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
      if (cmd === "library_list") return page([item("one")]);
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

describe("palette artifacts", () => {
  const skill = item("review-helper", { id: "a1" });

  function mockLibrary(
    respond: (text: string | null) => Promise<unknown> | unknown,
  ) {
    mockIPC((cmd, args) => {
      if (cmd === "library_list") {
        const q = (args as { query: { filter: { text: string | null } } })
          .query;
        return respond(q.filter.text);
      }
      if (cmd === "profiles_list") return [];
      return cmd === "app_info"
        ? { version: "1.2.3", dataDir: "/d" }
        : undefined;
    });
  }

  it("lists matching artifacts with kind and source, and opens one with Enter", async () => {
    mockLibrary((text) => page(text ? [skill] : []));
    const user = await openPalette();
    expect(screen.queryByRole("group", { name: "Artifacts" })).toBeNull();
    await user.type(screen.getByRole("combobox"), "review");
    const row = await screen.findByRole("option", { name: /review-helper/ });
    expect(row).toHaveTextContent("Skill · agent-skills");
    await user.hover(row);
    await user.keyboard("{Enter}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("navigates to the open route with Enter and click", async () => {
    mockLibrary(() => page([skill]));
    const seen: string[] = [];
    function Where() {
      const l = useLocation();
      seen.push(l.pathname + l.search);
      return null;
    }
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/profiles"]}>
        <Where />
        <AppRoutes />
      </MemoryRouter>,
    );
    await user.keyboard("{Control>}k{/Control}");
    await user.type(screen.getByRole("combobox"), "review");
    const row = await screen.findByRole("option", { name: /review-helper/ });
    await user.hover(row);
    await user.keyboard("{Enter}");
    expect(seen.at(-1)).toBe("/library?open=a1");
    await user.keyboard("{Control>}k{/Control}");
    await user.type(screen.getByRole("combobox"), "review");
    await user.click(
      await screen.findByRole("option", { name: /review-helper/ }),
    );
    expect(seen.at(-1)).toBe("/library?open=a1");
  });

  it("ignores a stale response", async () => {
    const slow = deferred();
    mockLibrary((text) =>
      text === "re" ? slow.promise : page([item("fresh-one", { id: "a2" })]),
    );
    const user = await openPalette();
    await user.type(screen.getByRole("combobox"), "re");
    await new Promise((r) => setTimeout(r, 200));
    await user.type(screen.getByRole("combobox"), "v");
    expect(
      await screen.findByRole("option", { name: /fresh-one/ }),
    ).toBeInTheDocument();
    slow.resolve(page([item("stale-one", { id: "a3" })]));
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.queryByRole("option", { name: /stale-one/ })).toBeNull();
  });
});

function deferred() {
  let resolve: (v: unknown) => void = () => undefined;
  const promise = new Promise((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
