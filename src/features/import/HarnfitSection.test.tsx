import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import { HarnfitSection } from "./HarnfitSection";

function cand(name: string, kind: "skill" | "plugin" = "skill"): Candidate {
  return {
    id: `id-${name}`,
    artifact: { kind, name } as Candidate["artifact"],
  } as Candidate;
}

function Where() {
  const at = useLocation();
  return (
    <p data-testid="where">
      {at.pathname}|{JSON.stringify(at.state)}
    </p>
  );
}

function setup(drawn: Record<number, boolean>, token: string | null = "tok") {
  mockIPC((command, args) => {
    if (command === "profiles_list")
      return Object.keys(drawn).map((id) => ({
        id: Number(id),
        name: `P${id}`,
      }));
    if (command === "harnmap_get") {
      const id = (args as { profileId: number }).profileId;
      if (id === 99) return Promise.reject("gone");
      return { drawn: drawn[id] ?? false };
    }
    return undefined;
  });
  render(
    <MemoryRouter initialEntries={["/import?x=1"]}>
      <Routes>
        <Route
          path="import"
          element={
            <HarnfitSection
              chosen={[cand("lite"), cand("suite", "plugin")]}
              token={token}
            />
          }
        />
        <Route path="*" element={<Where />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Harnfit in the import review", () => {
  it("offers profiles with a map, a button per candidate, and opens the preview fit", async () => {
    const user = userEvent.setup();
    setup({ 1: false, 2: true, 3: true, 99: true });
    expect(
      await screen.findByText(
        "Harnfit runs only when you click it. Fits are saved with the profile.",
      ),
    ).toBeVisible();
    const select = screen.getByLabelText("Fit into");
    expect(
      within(select)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["P2", "P3"]);
    expect(
      screen.getByRole("button", { name: "Harnfit suite" }),
    ).toBeDisabled();
    expect(screen.getByText("Harnfit for plugins comes later")).toBeVisible();
    await user.selectOptions(select, "3");
    await user.click(screen.getByRole("button", { name: "Harnfit lite" }));
    const where = (await screen.findByTestId("where")).textContent ?? "";
    expect(where).toContain("/profiles/3/harnfit/preview%3Aid-lite");
    expect(where).toContain(
      '"candidate":{"type":"preview","token":"tok","candidateId":"id-lite"}',
    );
    expect(where).toContain('"from":"/import?x=1"');
  });

  it("says so when no profile has a map", async () => {
    setup({ 1: false });
    expect(
      await screen.findByText(/No profile has a drawn map yet/),
    ).toBeVisible();
  });

  it("cannot fit without a preview", async () => {
    setup({ 2: true }, null);
    expect(
      await screen.findByRole("button", { name: "Harnfit lite" }),
    ).toBeDisabled();
  });
});
