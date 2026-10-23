import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { FileDiff } from "../../bindings/FileDiff";
import type { LibraryItem } from "../../bindings/LibraryItem";
import type { Query } from "../../bindings/Query";
import type { UpdateReport } from "../../bindings/UpdateReport";
import type { UpdateStatus } from "../../bindings/UpdateStatus";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const diff: FileDiff[] = [
  {
    path: "release/SKILL.md",
    status: "changed",
    hunks: [
      {
        oldStart: 3,
        newStart: 3,
        lines: [
          { kind: "context", text: "step changelog" },
          { kind: "removed", text: "step tag --sign" },
          { kind: "added", text: "step dry-run" },
        ],
      },
    ],
  },
  { path: "release/notes.md", status: "added", hunks: [] },
];

function detail(name: string): ArtifactDetail {
  return {
    id: `id-${name}`,
    artifact: {
      kind: "skill",
      name,
      description: null,
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
        kind: "git",
        location: "https://example.test/pack",
        branch: "main",
        subfolder: null,
        commitSha: "5e0b7aa1",
      },
      sourcePath: name,
      digest: null,
      importedAt: 1_700_000_000n,
      replacedSecretKeys: [],
    },
    files: [`${name}/SKILL.md`],
  };
}

function Probe() {
  return <span data-testid="search">{useLocation().search}</span>;
}

function report(
  id: string,
  outcome: UpdateReport["outcome"],
  over: Partial<UpdateReport> = {},
): UpdateReport {
  return {
    id,
    outcome,
    backup: null,
    fromVersion: null,
    toVersion: null,
    message: null,
    ...over,
  };
}

function setup(path: string, localEdits = false) {
  const calls: string[] = [];
  const queries: Query[] = [];
  const settings: Record<string, unknown> = {};
  let outdated = true;
  const status: UpdateStatus = {
    state: "outdated",
    fromVersion: "3.4.0",
    toVersion: "3.4.1",
    localEdits,
  };
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    const { key, value, choice, id } = a as {
      key?: string;
      value?: unknown;
      choice?: string;
      id?: string;
    };
    switch (cmd) {
      case "library_list": {
        const query = a.query as Query;
        queries.push(query);
        const rows: LibraryItem[] = [
          item("plain"),
          item("release", {
            update: outdated ? { toVersion: "3.4.1", localEdits } : null,
          }),
          item("noversion", { update: { toVersion: null, localEdits: false } }),
        ];
        const shown =
          query.filter.outdated || query.filter.statuses.includes("outdated")
            ? rows.filter((r) => r.update)
            : rows;
        return page(shown, {
          unfilteredTotal: rows.length,
          facets: {
            ...page([]).facets,
            sources: [
              {
                source: {
                  id: "s1",
                  kind: "git",
                  label: "acme/pack",
                  branch: null,
                },
                count: 3,
              },
            ],
            outdated: rows.filter((r) => r.update).length,
            statuses: [
              {
                status: "outdated",
                count: rows.filter((r) => r.update).length,
              },
            ],
          },
        });
      }
      case "tags_list":
      case "artifact_tags":
      case "artifact_collections":
      case "collections_list":
        return [];
      case "library_detail":
        return detail(String(id).replace("id-", ""));
      case "update_status":
        return outdated && id === "id-release" ? status : null;
      case "update_diff":
        return diff;
      case "update_apply":
        calls.push(`apply:${String(choice)}`);
        outdated = false;
        return report(String(id), choice === "keepMine" ? "kept" : "updated", {
          fromVersion: "3.4.0",
          toVersion: "3.4.1",
          backup:
            choice === "takeUpstream" && localEdits
              ? "/data/library/.backups/release/20261023T090000Z"
              : null,
        });
      case "update_all":
        calls.push("all");
        return [
          report("id-noversion", "updated"),
          report("id-release", "skippedLocalEdits"),
        ];
      case "updates_check":
        calls.push("check");
        return {
          sources: [
            { id: "s1", result: { type: "unreachable", reason: "offline" } },
          ],
        };
      case "settings_get":
        return settings[String(key)] ?? null;
      case "settings_set":
        settings[String(key)] = value;
        return undefined;
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <Probe />
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return { calls, queries, settings };
}

describe("updates in the library", () => {
  it("shows the badge, with and without a version, and says the version only once", async () => {
    setup("/library");
    expect(await screen.findByText("Update 3.4.1")).toBeInTheDocument();
    expect(screen.queryByText("→ 3.4.1")).not.toBeInTheDocument();
    expect(screen.queryByText("changed")).not.toBeInTheDocument();
    expect(screen.getByText("Update")).toBeInTheDocument();
  });

  it("filters to outdated artifacts with the Outdated value of the Status group", async () => {
    const user = userEvent.setup();
    const { queries } = setup("/library");
    await screen.findByText("Update 3.4.1");
    await user.click(screen.getByRole("button", { name: /^Outdated/ }));
    await waitFor(() =>
      expect(screen.getByTestId("search")).toHaveTextContent("status=outdated"),
    );
    await waitFor(() =>
      expect(queries.at(-1)?.filter.statuses).toEqual(["outdated"]),
    );
    await waitFor(() => expect(screen.queryByText("plain")).toBeNull());
    expect(
      screen.getByRole("button", { name: "Remove filter Outdated" }),
    ).toBeInTheDocument();
  });

  it("checks for updates, stores the time and shows a dismissible per-source error", async () => {
    const user = userEvent.setup();
    const { calls, settings } = setup("/library");
    await screen.findByText("Update 3.4.1");
    expect(screen.getByText("Never checked")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("acme/pack: offline")).toBeInTheDocument();
    expect(calls).toContain("check");
    expect(typeof settings["updates.lastCheck"]).toBe("string");
    expect(screen.queryByText("Never checked")).toBeNull();
    await user.click(
      screen.getByRole("button", { name: "Dismiss the update errors" }),
    );
    expect(screen.queryByText("acme/pack: offline")).toBeNull();
  });

  it("updates an unedited artifact from the drawer callout", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library?open=id-release");
    const panel = await screen.findByRole("dialog", {
      name: "release details",
    });
    expect(
      await within(panel).findByText("Update available: 3.4.0 → 3.4.1"),
    ).toBeInTheDocument();
    expect(within(panel).getByText("3.4.1 available")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Update" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Update release",
    });
    expect(
      await within(dialog).findByText("release/SKILL.md"),
    ).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("button", {
        name: "Show changes in release/SKILL.md",
      }),
    );
    expect(within(dialog).getByText(/step dry-run/)).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("button", { name: "Update library copy" }),
    );
    expect(await within(dialog).findByRole("status")).toHaveTextContent(
      "Updated the library copy to 3.4.1",
    );
    expect(calls).toEqual(["apply:takeUpstream"]);
    await waitFor(() =>
      expect(
        within(panel).queryByText("Update available: 3.4.0 → 3.4.1"),
      ).toBeNull(),
    );
  });

  it("shows Yours against Upstream and the backup path after taking upstream", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library?open=id-release", true);
    const panel = await screen.findByRole("dialog", {
      name: "release details",
    });
    expect(
      await within(panel).findByText(/You edited this artifact locally/),
    ).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Update" }));
    const dialog = await screen.findByRole("dialog", {
      name: "release: your edits and upstream",
    });
    expect(
      await within(dialog).findByText("Yours (edited locally)"),
    ).toBeInTheDocument();
    expect(within(dialog).getByText("Upstream")).toBeInTheDocument();
    expect(within(dialog).getByText("step tag --sign")).toBeInTheDocument();
    expect(within(dialog).getByText("step dry-run")).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("radio", { name: /Take upstream 3.4.1/ }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Confirm" }));
    expect(
      await within(dialog).findByText(/20261023T090000Z/),
    ).toBeInTheDocument();
    expect(calls).toEqual(["apply:takeUpstream"]);
  });

  it("keeps the user's version", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library?open=id-release", true);
    const panel = await screen.findByRole("dialog", {
      name: "release details",
    });
    await user.click(
      await within(panel).findByRole("button", { name: "Update" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "release: your edits and upstream",
    });
    await within(dialog).findByText("Yours (edited locally)");
    await user.click(within(dialog).getByRole("button", { name: "Confirm" }));
    expect(await within(dialog).findByRole("status")).toHaveTextContent(
      "Kept your version",
    );
    expect(calls).toEqual(["apply:keepMine"]);
  });

  it("keeps Update all secondary so Import stays the one primary button", async () => {
    setup("/library");
    const button = await screen.findByRole("button", {
      name: /Update all \(2\)/,
    });
    expect(button.className).not.toMatch(/primary/);
  });

  it("confirms Update all and lists the artifacts skipped for local edits", async () => {
    const user = userEvent.setup();
    const { calls } = setup("/library", true);
    await screen.findByText("Update 3.4.1");
    await user.click(screen.getByRole("button", { name: /Update all \(2\)/ }));
    const dialog = await screen.findByRole("dialog", { name: "Update all" });
    expect(
      await within(dialog).findByText(/release has local edits and is skipped/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Update 1" }));
    expect(
      await within(dialog).findByText(/Skipped for local edits: release/),
    ).toBeInTheDocument();
    expect(calls).toEqual(["all"]);
  });
});
