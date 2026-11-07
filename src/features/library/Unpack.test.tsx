import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { UnpackOutcome } from "../../bindings/UnpackOutcome";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

function detailOf(
  id: string,
  name: string,
  kind: ArtifactKind,
): ArtifactDetail {
  return {
    id,
    artifact: {
      kind,
      name,
      description: `About ${name}`,
      path: name,
      files: [],
      metadata: {},
      references: [],
      diagnostics: [],
      digest: null,
      origin: { type: "own" },
    },
    audit: null,
    auditedAt: null,
    links: { needs: [], neededBy: [] },
    manualOnly: false,
    provenance: {
      source: {
        id: "s1",
        kind: "local",
        location: "/home/me/kit",
        branch: null,
        subfolder: null,
        commitSha: null,
      },
      sourcePath: name,
      digest: null,
      importedAt: 1_700_000_000n,
      replacedSecretKeys: [],
    },
    files: [],
  };
}

const unpackable: UnpackOutcome = {
  refused: null,
  reason: null,
  parts: [
    { id: "id-alpha", name: "alpha", kind: "skill" },
    { id: "id-beta", name: "beta", kind: "skill" },
  ],
  profiles: [
    { id: 1n, name: "Heavy", from: 4, to: 5 },
    { id: 2n, name: "Light", from: 2, to: 3 },
  ],
  placementsDropped: 1,
};

const stays: UnpackOutcome = {
  refused: { type: "wholeOnlyParts", parts: ["hooks"] },
  reason: "Has hooks: they only work inside the plugin.",
  parts: [],
  profiles: [],
  placementsDropped: 0,
};

function show(
  kind: ArtifactKind,
  plan: UnpackOutcome,
  calls: string[] = [],
  failUnpack = false,
) {
  mockIPC((cmd, args) => {
    calls.push(cmd);
    const a = (args ?? {}) as Record<string, string>;
    switch (cmd) {
      case "library_list":
        return page([item("pack", {}, kind)]);
      case "library_detail":
        return detailOf(
          a.id ?? "",
          a.id === "id-alpha" ? "alpha" : "pack",
          kind,
        );
      case "library_unpack_plan":
        return plan;
      case "library_unpack_plugin":
        if (failUnpack) throw new Error("disk full");
        return plan;
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  return render(
    <MemoryRouter initialEntries={["/library?open=id-pack"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("unpacking a whole plugin", () => {
  it("lists the parts and the profiles in the confirm, then opens the first part", async () => {
    const calls: string[] = [];
    show("plugin", unpackable, calls);
    const panel = await screen.findByRole("dialog", { name: "pack details" });
    await userEvent
      .setup()
      .click(await within(panel).findByRole("button", { name: /^Unpack$/ }));
    const confirm = await screen.findByRole("dialog", { name: "Unpack pack" });
    expect(
      within(confirm).getByText(/Becomes 2 artifacts: alpha, beta\./),
    ).toBeInTheDocument();
    expect(
      within(confirm).getByText(/Heavy v4 → v5, Light v2 → v3/),
    ).toBeInTheDocument();
    expect(
      within(confirm).getByText("Keeps its tags and collections."),
    ).toBeInTheDocument();
    expect(within(confirm).getByText(/lose the/)).toHaveTextContent(
      "Its skills lose the plugin: prefix; a project keeps its version until you switch it.",
    );
    await userEvent
      .setup()
      .click(within(confirm).getByRole("button", { name: "Unpack" }));
    await waitFor(() => expect(calls).toContain("library_unpack_plugin"));
    expect(
      await screen.findByRole("dialog", { name: "alpha details" }),
    ).toBeInTheDocument();
  });

  it("changes nothing when the confirm is cancelled", async () => {
    const calls: string[] = [];
    show("plugin", unpackable, calls);
    const panel = await screen.findByRole("dialog", { name: "pack details" });
    const user = userEvent.setup();
    await user.click(
      await within(panel).findByRole("button", { name: /^Unpack$/ }),
    );
    await user.click(
      within(
        await screen.findByRole("dialog", { name: "Unpack pack" }),
      ).getByRole("button", { name: "Cancel" }),
    );
    expect(calls).not.toContain("library_unpack_plugin");
    expect(screen.queryByRole("dialog", { name: "Unpack pack" })).toBeNull();
  });

  it("says a plugin with no profile holds it, and shows a failure in the confirm", async () => {
    show(
      "plugin",
      { ...unpackable, profiles: [], placementsDropped: 0 },
      [],
      true,
    );
    const panel = await screen.findByRole("dialog", { name: "pack details" });
    const user = userEvent.setup();
    await user.click(
      await within(panel).findByRole("button", { name: /^Unpack$/ }),
    );
    const confirm = await screen.findByRole("dialog", { name: "Unpack pack" });
    expect(
      within(confirm).getByText("No profile holds it."),
    ).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Unpack" }));
    expect(await within(confirm).findByRole("alert")).toHaveTextContent(
      "disk full",
    );
  });

  it("offers no button for a plugin that must stay whole, only the reason", async () => {
    show("plugin", stays);
    const panel = await screen.findByRole("dialog", { name: "pack details" });
    expect(
      await within(panel).findByText(
        "Stays whole: Has hooks: they only work inside the plugin.",
      ),
    ).toBeInTheDocument();
    expect(
      within(panel).queryByRole("button", { name: /^Unpack$/ }),
    ).toBeNull();
  });

  it("does not ask about artifacts that are not plugins", async () => {
    const calls: string[] = [];
    show("skill", unpackable, calls);
    const panel = await screen.findByRole("dialog", { name: "pack details" });
    await within(panel).findByRole("button", { name: /Find bundles/ });
    expect(
      within(panel).queryByRole("button", { name: /^Unpack$/ }),
    ).toBeNull();
    expect(calls).not.toContain("library_unpack_plan");
  });
});
