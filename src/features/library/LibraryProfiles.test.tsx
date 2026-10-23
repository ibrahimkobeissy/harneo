import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { describe, expect, it } from "vitest";
import type { ArtifactDetail } from "../../bindings/ArtifactDetail";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { Query } from "../../bindings/Query";
import { CollectionsProvider } from "../../app/collections";
import { item, page } from "../../test/library";
import { CollectionScreen, LibraryScreen } from ".";

function detail(
  name: string,
  kind: ArtifactKind = "skill",
  manualOnly = false,
): ArtifactDetail {
  return {
    id: `id-${name}`,
    artifact: {
      kind,
      name,
      description: `About ${name}`,
      path: `${name}`,
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
    manualOnly,
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

interface Spec {
  items: string[];
  settings: string | null;
  mcp: { artifact: string; env: Record<string, string> }[];
  manualOnly: Record<string, boolean>;
}
const emptySpec: Spec = { items: [], settings: null, mcp: [], manualOnly: {} };

function Probe() {
  const where = useLocation();
  return <span data-testid="where">{where.pathname + where.search}</span>;
}

interface Backend {
  fail?: Record<string, string>;
  using?: { id: number; name: string; version: number }[];
  spec?: Spec;
  /** Artifacts a resolve returns for a spec with this many items. */
  extra?: number;
  details?: ArtifactDetail[];
}

function setup(path: string, backend: Backend = {}) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  const queries: Query[] = [];
  const details = backend.details ?? [
    detail("alpha"),
    detail("beta", "agent"),
    detail("conf", "settings"),
    detail("api", "mcp"),
  ];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    const failure = backend.fail?.[cmd];
    if (failure) throw new Error(failure);
    switch (cmd) {
      case "library_list":
        queries.push(a.query as Query);
        return page(
          details.map((d) => item(d.artifact.name)),
          {
            facets: {
              kinds: [],
              tags: [],
              sources: [],
              tiers: [],
              withFindings: 0,
              outdated: 0,
              statuses: [],
              auto: 3,
              manualOnly: 1,
            },
          },
        );
      case "collection_page":
        queries.push(a.query as Query);
        return page([item("alpha")]);
      case "collections_list":
        return [
          {
            id: 4n,
            name: "Review kit",
            kind: "manual",
            rule: null,
            count: 1,
          },
        ];
      case "library_detail":
        return details.find((d) => d.id === a.id);
      case "artifact_profiles":
        return backend.using ?? [];
      case "profiles_list":
        return [
          {
            id: 2,
            name: "Light",
            description: "",
            version: 3,
            counts: [],
            fileCount: 0,
            projectCount: 0,
            updatedAt: 0n,
          },
        ];
      case "profile_get":
        return {
          id: 2,
          name: "Light",
          description: "",
          version: 3,
          currentVersion: 3,
          spec: backend.spec ?? emptySpec,
          versions: [],
          createdAt: 0n,
          updatedAt: 0n,
        };
      case "profile_resolve": {
        const spec = a.spec as Spec;
        const n = spec.items.length + spec.mcp.length + (spec.settings ? 1 : 0);
        const count = n === 0 ? 0 : n + (backend.extra ?? 0);
        return {
          artifacts: Array.from({ length: count }, (_, i) => ({ id: `x${i}` })),
          missing: [],
          worstTrust: null,
        };
      }
      case "profile_save":
        return 4;
      case "profile_from_collection":
        return 7;
      case "tags_list":
      case "artifact_tags":
      case "artifact_collections":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[path]}>
      <CollectionsProvider>
        <Probe />
        <Routes>
          <Route path="/library" element={<LibraryScreen />} />
          <Route path="/collections/:id" element={<CollectionScreen />} />
          <Route path="/profiles/:id" element={<span>builder</span>} />
        </Routes>
      </CollectionsProvider>
    </MemoryRouter>,
  );
  const sent = (cmd: string) => calls.filter((c) => c.cmd === cmd);
  return { sent, queries };
}

async function panel(name = "alpha") {
  return screen.findByRole("dialog", { name: `${name} details` });
}

describe("Used in", () => {
  it("lists the profiles with their versions, linking to the builder", async () => {
    setup("/library?open=id-alpha", {
      using: [
        { id: 2, name: "Light", version: 3 },
        { id: 5, name: "Heavy", version: 1 },
      ],
    });
    const link = await within(await panel()).findByRole("link", {
      name: /Light/,
    });
    expect(link).toHaveAttribute("href", "/profiles/2");
    expect(link).toHaveTextContent("v3");
    expect(screen.getByRole("link", { name: /Heavy/ })).toHaveTextContent("v1");
  });

  it("says when the artifact is in no profile", async () => {
    setup("/library?open=id-alpha");
    expect(
      await within(await panel()).findByText("Not in any profile yet."),
    ).toBeInTheDocument();
  });

  it("shows the failure", async () => {
    setup("/library?open=id-alpha", { fail: { artifact_profiles: "no db" } });
    expect(await within(await panel()).findByRole("alert")).toHaveTextContent(
      "no db",
    );
  });
});

describe("Manual only", () => {
  it("toggles on and off with the prototype's messages", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/library?open=id-alpha");
    const p = await panel();
    const toggle = await within(p).findByRole("switch", {
      name: "Manual only",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    expect(await within(p).findByRole("status")).toHaveTextContent(
      "alpha is now manual only",
    );
    expect(toggle).toHaveAttribute("aria-checked", "true");
    expect(sent("artifact_set_manual_only")[0]?.args).toEqual({
      id: "id-alpha",
      manualOnly: true,
    });
    await user.click(toggle);
    await waitFor(() =>
      expect(within(p).getByRole("status")).toHaveTextContent(
        "alpha can be auto-invoked again",
      ),
    );
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  it("starts on for a manual-only skill", async () => {
    setup("/library?open=id-alpha", {
      details: [detail("alpha", "skill", true)],
    });
    expect(
      await within(await panel()).findByRole("switch", { name: "Manual only" }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("is offered on skills only", async () => {
    setup("/library?open=id-beta");
    await within(await panel("beta")).findByText("Used in");
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  });

  it("shows a failure and keeps the switch where it was", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha", {
      fail: { artifact_set_manual_only: "read only" },
    });
    const p = await panel();
    const toggle = await within(p).findByRole("switch");
    await user.click(toggle);
    expect(await within(p).findByRole("alert")).toHaveTextContent("read only");
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });
});

describe("Add to… → Profile", () => {
  async function openMenu(
    user: ReturnType<typeof userEvent.setup>,
    name = "alpha",
  ) {
    const p = await panel(name);
    await user.click(await within(p).findByRole("button", { name: /Add to/ }));
    return within(await screen.findByRole("menu", { name: "Add to" }));
  }

  it("groups the menu under Collections and Profiles, without a Profile: prefix", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha");
    const menu = await openMenu(user);
    expect(await menu.findByText("Collections")).toBeInTheDocument();
    expect(menu.getByText("Profiles")).toBeInTheDocument();
    const group = menu.getByRole("group", { name: "Profiles" });
    expect(
      within(group).getByRole("menuitem", { name: "Light" }),
    ).toBeVisible();
    expect(menu.queryByText(/^Profile:/)).toBeNull();
  });

  it("adds the artifact and says how many came bundled", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/library?open=id-alpha", { extra: 2 });
    const menu = await openMenu(user);
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    expect(await menu.findByRole("status")).toHaveTextContent(
      "Added alpha + 2 bundled to Light (now v4).",
    );
    expect(sent("profile_save")[0]?.args).toEqual({
      id: 2,
      spec: { ...emptySpec, items: ["id-alpha"] },
    });
    // Used in follows, and the menu now marks the profile.
    await waitFor(() =>
      expect(sent("artifact_profiles").length).toBeGreaterThan(1),
    );
  });

  it("leaves out the bundle count when nothing came with it", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha");
    const menu = await openMenu(user);
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    expect(await menu.findByRole("status")).toHaveTextContent(
      "Added alpha to Light (now v4).",
    );
  });

  it("says so when the artifact is already in the profile", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/library?open=id-alpha", {
      using: [{ id: 2, name: "Light", version: 3 }],
    });
    const menu = await openMenu(user);
    expect(await menu.findByText("in profile")).toBeInTheDocument();
    await user.click(menu.getByRole("menuitem", { name: /Light/ }));
    expect(await menu.findByRole("status")).toHaveTextContent(
      "Already in Light",
    );
    expect(sent("profile_save")).toHaveLength(0);
  });

  it("puts a Settings template and an MCP server where they belong", async () => {
    const user = userEvent.setup();
    const first = setup("/library?open=id-conf");
    const menu = await openMenu(user, "conf");
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    await menu.findByText(/Added conf to Light/);
    expect(first.sent("profile_save")[0]?.args).toEqual({
      id: 2,
      spec: { ...emptySpec, settings: "id-conf" },
    });
  });

  it("adds an MCP server with no environment", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/library?open=id-api");
    const menu = await openMenu(user, "api");
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    await menu.findByText(/Added api to Light/);
    expect(sent("profile_save")[0]?.args).toEqual({
      id: 2,
      spec: { ...emptySpec, mcp: [{ artifact: "id-api", env: {} }] },
    });
  });

  it("does not replace a Settings template the profile already has", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/library?open=id-conf", {
      spec: { ...emptySpec, settings: "other" },
    });
    const menu = await openMenu(user, "conf");
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    expect(await menu.findByRole("status")).toHaveTextContent(
      "Light already has a Settings template",
    );
    expect(sent("profile_save")).toHaveLength(0);
  });

  it("shows a save failure", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha", { fail: { profile_save: "locked" } });
    const menu = await openMenu(user);
    await user.click(await menu.findByRole("menuitem", { name: /Light/ }));
    expect(await menu.findByRole("alert")).toHaveTextContent("locked");
  });

  it("shows a failure to list the profiles", async () => {
    const user = userEvent.setup();
    setup("/library?open=id-alpha", { fail: { profiles_list: "no list" } });
    const menu = await openMenu(user);
    expect(await menu.findByRole("alert")).toHaveTextContent("no list");
  });
});

describe("Invocation filter", () => {
  it("offers Auto and Manual only with their counts and filters on one", async () => {
    const user = userEvent.setup();
    const { queries } = setup("/library");
    const manual = await screen.findByRole("button", { name: /Manual only/ });
    expect(manual).toHaveTextContent("1");
    expect(screen.getByRole("button", { name: /^Auto/ })).toHaveTextContent(
      "3",
    );
    await user.click(manual);
    await waitFor(() =>
      expect(screen.getByTestId("where")).toHaveTextContent("inv=manual"),
    );
    await waitFor(() => expect(queries.at(-1)?.filter.manualOnly).toBe(true));
    await user.click(screen.getByRole("button", { name: /^Auto/ }));
    await waitFor(() =>
      expect(queries.at(-1)?.filter.manualOnly ?? null).toBeNull(),
    );
    await user.click(
      screen.getByRole("button", { name: "Clear invocation filter" }),
    );
    await user.click(screen.getByRole("button", { name: /^Auto/ }));
    await waitFor(() => expect(queries.at(-1)?.filter.manualOnly).toBe(false));
  });

  it("reads the filter from the URL", async () => {
    const { queries } = setup("/library?inv=manual");
    await waitFor(() => expect(queries.at(-1)?.filter.manualOnly).toBe(true));
  });
});

describe("Create profile from a collection", () => {
  it("makes a profile named after the collection and opens it", async () => {
    const user = userEvent.setup();
    const { sent } = setup("/collections/4");
    await user.click(
      await screen.findByRole("button", { name: "Create profile" }),
    );
    await waitFor(() =>
      expect(screen.getByTestId("where")).toHaveTextContent("/profiles/7"),
    );
    expect(sent("profile_from_collection")[0]?.args).toEqual({
      collectionId: 4,
      name: "Review kit",
    });
  });

  it("shows why it could not", async () => {
    const user = userEvent.setup();
    setup("/collections/4", {
      fail: { profile_from_collection: "a profile named Review kit exists" },
    });
    await user.click(
      await screen.findByRole("button", { name: "Create profile" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("exists");
    expect(
      screen.getByRole("button", { name: "Create profile" }),
    ).toBeEnabled();
  });
});
