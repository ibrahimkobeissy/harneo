import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactFit } from "../../bindings/ArtifactFit";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import { item, page } from "../../test/library";
import { HarnfitMenu } from "./HarnfitMenu";
import { LibraryScreen } from ".";

const profiles = [
  { id: 7n, name: "Heavy", category: "software" },
  { id: 8n, name: "Light", category: "research" },
];

function fit(over: Partial<ArtifactFit> = {}): ArtifactFit {
  return {
    profileId: 7n,
    profileName: "Heavy",
    verdict: "conflicts",
    createdAt: 1_730_000_000,
    stale: false,
    ...over,
  };
}

function Where() {
  const at = useLocation();
  return (
    <p data-testid="where">
      {at.pathname}|{JSON.stringify(at.state)}
    </p>
  );
}

function mock(fits: ArtifactFit[]) {
  mockIPC((cmd) => {
    switch (cmd) {
      case "profiles_list":
        return profiles;
      case "harnfit_for_artifact":
        return fits;
      case "templates_list":
        return [
          { id: "software", label: "Software development" },
          { id: "research", label: "Research" },
        ];
      default:
        return undefined;
    }
  });
}

function menu(kind: ArtifactKind = "skill") {
  render(
    <MemoryRouter initialEntries={["/library?view=cards"]}>
      <Routes>
        <Route
          path="library"
          element={
            <HarnfitMenu
              artifactId="a1"
              artifactName="tdd-lite"
              artifactKind={kind}
            />
          }
        />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Harnfit in… menu", () => {
  it("lists the profiles with category, saved verdict or not fitted yet, and opens the right fit", async () => {
    mock([fit({ stale: true })]);
    const user = userEvent.setup();
    menu();
    await user.click(screen.getByRole("button", { name: /Harnfit in/ }));
    const heavy = await screen.findByRole("menuitem", { name: /Heavy/ });
    expect(within(heavy).getByText(/Software development/)).toBeVisible();
    expect(within(heavy).getByText("Conflicts")).toBeVisible();
    expect(within(heavy).getByText(/fit saved .*out of date/)).toBeVisible();
    const light = screen.getByRole("menuitem", { name: /Light/ });
    expect(within(light).getByText("not fitted yet")).toBeVisible();
    await user.click(light);
    const where = (await screen.findByTestId("where")).textContent ?? "";
    expect(where).toContain("/profiles/8/harnfit/lib%3Aa1");
    expect(where).toContain('"from":"/library?view=cards"');
    expect(where).toContain('"name":"tdd-lite"');
  });

  it("disables every profile for a plugin taken whole", async () => {
    mock([]);
    const user = userEvent.setup();
    menu("plugin");
    await user.click(screen.getByRole("button", { name: /Harnfit in/ }));
    expect(
      await screen.findByText("Harnfit for plugins comes later"),
    ).toBeVisible();
    for (const row of await screen.findAllByRole("menuitem"))
      expect(row).toBeDisabled();
  });

  it("closes on Escape and says when there are no profiles", async () => {
    mockIPC(() => []);
    const user = userEvent.setup();
    menu();
    await user.click(screen.getByRole("button", { name: /Harnfit in/ }));
    expect(await screen.findByText("No profiles yet.")).toBeVisible();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("Library card badges", () => {
  it("shows a verdict badge per profile with a fit, stale ones out of date", async () => {
    mockIPC((cmd, args) => {
      switch (cmd) {
        case "library_list":
          return page([item("alpha"), item("beta")]);
        case "harnfit_for_artifact":
          return (args as { artifactId: string }).artifactId === "id-alpha"
            ? [
                fit(),
                fit({
                  profileId: 8n,
                  profileName: "Light",
                  verdict: "fits",
                  stale: true,
                }),
                fit({
                  profileId: 9n,
                  profileName: "Third",
                  verdict: "fitsWithChanges",
                }),
              ]
            : (undefined as unknown as ArtifactFit[]);
        default:
          return undefined;
      }
    });
    render(
      <MemoryRouter initialEntries={["/library"]}>
        <Routes>
          <Route path="library" element={<LibraryScreen />} />
        </Routes>
      </MemoryRouter>,
    );
    const badges = await screen.findByLabelText("Harnfit verdicts");
    expect(within(badges).getByText("Heavy: Conflicts")).toBeVisible();
    expect(within(badges).getByText("Light: Fits · out of date")).toBeVisible();
    expect(within(badges).getByText("+1")).toBeVisible();
    await waitFor(() =>
      expect(screen.getAllByLabelText("Harnfit verdicts")).toHaveLength(1),
    );
  });
});
