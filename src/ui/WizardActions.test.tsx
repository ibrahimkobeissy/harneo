import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { AppRoutes } from "../App";
import { Button, Screen, WizardActions } from ".";

beforeEach(() => {
  mockIPC((cmd) =>
    cmd === "app_info"
      ? { version: "1.2.3", dataDir: "/d" }
      : cmd === "profiles_list" || cmd === "projects_list"
        ? []
        : undefined,
  );
});

describe("the wizard action bar", () => {
  it("sits after the scrolling body, with back before the primary action", () => {
    render(
      <Screen
        title="Wizard"
        foot={
          <WizardActions back={<Button>Back</Button>}>
            <Button variant="primary">Go</Button>
          </WizardActions>
        }
      >
        <p>content</p>
      </Screen>,
    );
    const section = screen.getByRole("region", { name: "Wizard" });
    const body = section.querySelector("[data-screen-body]");
    const foot = section.querySelector("[data-screen-foot]");
    expect(body?.contains(foot)).toBe(false);
    expect(body?.nextElementSibling).toBe(foot);
    const back = screen.getByRole("button", { name: "Back" });
    const go = screen.getByRole("button", { name: "Go" });
    expect(foot?.contains(back) && foot.contains(go)).toBe(true);
    expect(back.compareDocumentPosition(go)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it.each(["/import", "/scaffold", "/refit"])(
    "%s keeps its Continue button outside the scrolling body",
    async (path) => {
      render(
        <MemoryRouter initialEntries={[path]}>
          <AppRoutes />
        </MemoryRouter>,
      );
      const go = await screen.findByRole("button", { name: "Continue" });
      expect(go.closest("[data-screen-foot]")).not.toBeNull();
      expect(go.closest("[data-screen-body]")).toBeNull();
    },
  );
});
