import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { ReplaceOutcome } from "../../bindings/ReplaceOutcome";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";
import { ItemCard } from "./ItemCard";

const REASON = "Has hooks: they only work inside the plugin.";

const detail = (flagged: boolean): ArtifactDetail => ({
  id: "id-alpha",
  artifact: {
    kind: "skill",
    name: "alpha",
    description: "About alpha",
    path: "skills/alpha",
    files: [],
    metadata: {
      fromPlugin: { name: "impeccable", folder: "", marketplace: null },
    },
    references: [],
    diagnostics: [],
    digest: null,
    origin: { type: "own" },
  },
  audit: null,
  auditedAt: null,
  links: { needs: [], neededBy: [] },
  manualOnly: false,
  ...(flagged ? { mustStayWhole: REASON } : {}),
  provenance: {
    source: {
      id: "s1",
      kind: "local",
      location: "/home/me/kit",
      branch: null,
      subfolder: null,
      commitSha: null,
    },
    sourcePath: "skills/alpha",
    digest: null,
    importedAt: 1_700_000_000n,
    replacedSecretKeys: [],
  },
  files: [],
});

const outcome: ReplaceOutcome = {
  plugin: "impeccable",
  pluginId: "id-impeccable",
  reason: REASON,
  parts: [
    { id: "id-alpha", name: "alpha", kind: "skill" },
    { id: "id-beta", name: "beta", kind: "skill" },
  ],
  profiles: [{ id: 1n, name: "Heavy", from: 4, to: 5 }],
  placementsDropped: 0,
};

function show(flagged: boolean, calls: string[] = []) {
  mockIPC((cmd) => {
    calls.push(cmd);
    switch (cmd) {
      case "library_list":
        return page([item("alpha", { fromPlugin: "impeccable" })]);
      case "library_detail":
        return detail(flagged);
      case "library_replace_plan":
      case "library_replace_with_whole":
        return outcome;
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  return render(
    <MemoryRouter initialEntries={["/library?open=id-alpha"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("parts of a plugin that must now stay whole", () => {
  it("marks the card with the reason", () => {
    const part = item("alpha", {
      fromPlugin: "impeccable",
      mustStayWhole: REASON,
    });
    render(<ItemCard item={part} onOpen={() => {}} />);
    expect(screen.getByText("Stays whole")).toHaveAttribute(
      "title",
      `Taken apart from impeccable, which only works whole: ${REASON}`,
    );
  });

  it("marks no card of a skills-only plugin", () => {
    render(
      <ItemCard
        item={item("alpha", { fromPlugin: "impeccable" })}
        onOpen={() => {}}
      />,
    );
    expect(screen.queryByText("Stays whole")).toBeNull();
  });

  it("names the reason in the panel and replaces the parts after a confirm", async () => {
    const calls: string[] = [];
    show(true, calls);
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    expect(
      await within(panel).findByText(
        `Taken apart from impeccable, which only works whole: ${REASON}`,
      ),
    ).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(
      within(panel).getByRole("button", {
        name: "Replace with the whole plugin",
      }),
    );
    const confirm = await screen.findByRole("dialog", {
      name: "Replace the parts of impeccable",
    });
    expect(within(confirm).getByText(/alpha, beta/)).toBeInTheDocument();
    expect(within(confirm).getByText(/Heavy v4 → v5/)).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Replace" }));
    await waitFor(() => expect(calls).toContain("library_replace_with_whole"));
  });

  it("shows no notice in the panel of a part that can stay a part", async () => {
    show(false);
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await within(panel).findByRole("button", { name: /Find bundles/ });
    expect(
      within(panel).queryByRole("button", {
        name: "Replace with the whole plugin",
      }),
    ).toBeNull();
  });
});
