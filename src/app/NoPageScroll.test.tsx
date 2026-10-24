import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AppRoutes } from "../App";
import { item, page } from "../test/library";

beforeEach(() => {
  mockIPC((cmd) => {
    if (cmd === "library_list") return page([item("one")]);
    if (cmd === "library_audits" || cmd === "profiles_list") return [];
    return cmd === "app_info" ? { version: "1.2.3", dataDir: "/d" } : undefined;
  });
});

const routes = [
  "/library",
  "/profiles",
  "/projects",
  "/security",
  "/cleanup",
  "/import",
  "/scaffold",
  "/refit",
  "/settings",
];

describe("the document never scrolls", () => {
  it.each(routes)(
    "%s renders its screen inside the framed main",
    async (path) => {
      render(
        <MemoryRouter initialEntries={[path]}>
          <AppRoutes />
        </MemoryRouter>,
      );
      const h1 = await screen.findByRole("heading", { level: 1 });
      const section = h1.closest("section");
      expect(section?.closest("main#main")).not.toBeNull();
      expect(section?.querySelector("[data-screen-head]")).not.toBeNull();
      expect(section?.querySelector("[data-screen-body]")).not.toBeNull();
      expect(
        (document.scrollingElement ?? document.documentElement).scrollTop,
      ).toBe(0);
    },
  );
});
