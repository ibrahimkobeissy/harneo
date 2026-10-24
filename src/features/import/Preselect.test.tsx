import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Candidate } from "../../bindings/Candidate";
import { ImportScreen } from ".";

function candidate(name: string): Candidate {
  return {
    id: `id-${name}`,
    artifact: {
      kind: "skill",
      name,
      description: `About ${name}`,
      path: `skills/${name}`,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    alreadyInLibrary: false,
    replacedSecretKeys: [],
    suggestedTags: [],
    copies: [],
    audit: {
      score: 100,
      tier: "trusted",
      findings: [],
      reach: { files: [], network: [], shell: [], env: [], protects: 0 },
      dismissed: [],
      confirmation: null,
    },
  };
}

function setup(search: string) {
  mockIPC((cmd) => {
    if (cmd === "home_claude_dir") return "/home/me/.claude";
    if (cmd === "library_preview") {
      return {
        source: { kind: "local", path: "/work/shop" },
        diagnostics: [],
        candidates: [candidate("alpha"), candidate("beta")],
      };
    }
    return null;
  });
  render(
    <MemoryRouter initialEntries={[`/import${search}`]}>
      <Routes>
        <Route path="/import" element={<ImportScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("Import preselection from the Drift tab", () => {
  it("imports only the artifact named by ?select=", async () => {
    setup("?path=%2Fwork%2Fshop&select=id-beta");
    const u = userEvent.setup();
    await u.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByText(/Found 2 artifacts/);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByRole("button", { name: /Import 1 artifact/ }),
    ).toBeEnabled();
  });

  it("ignores a select that matches no candidate", async () => {
    setup("?path=%2Fwork%2Fshop&select=nope");
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Continue" }));
    const u = userEvent.setup();
    await screen.findByText(/Found 2 artifacts/);
    await u.click(screen.getByRole("button", { name: "Continue" }));
    expect(
      await screen.findByRole("button", { name: /Import 2 artifacts/ }),
    ).toBeEnabled();
  });
});
