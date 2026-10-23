import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createMemoryRouter,
  Link,
  Outlet,
  RouterProvider,
  useLocation,
} from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { ProfileSpec } from "../../bindings/ProfileSpec";
import type { ResolvedItem } from "../../bindings/ResolvedItem";
import { dropDraft } from "./drafts";
import { ProfileBuilder } from ".";

interface Art {
  id: string;
  kind: ArtifactKind;
  name: string;
  trust?: number | null;
  findings?: number;
  bundle?: string[];
  files?: string[];
  metadata?: Record<string, unknown>;
  description?: string;
}

const lib: Art[] = [
  { id: "L", kind: "skill", name: "review", trust: 90, bundle: ["M"] },
  { id: "M", kind: "agent", name: "reviewer", trust: 88 },
  { id: "T", kind: "skill", name: "tdd", trust: 40, findings: 3 },
  { id: "D", kind: "skill", name: "dup", trust: null },
  { id: "R1", kind: "rule", name: "r-one" },
  { id: "R2", kind: "rule", name: "r-two" },
  { id: "R3", kind: "rule", name: "r-three" },
  {
    id: "S",
    kind: "settings",
    name: "strict",
    description: "Locked down",
    files: ["settings.json", "hooks/guard.sh"],
    metadata: {
      permissions: { deny: ["a", "b"], ask: ["c"], allow: ["d", "e", "f"] },
      hooks: { PreToolUse: [{ matcher: "Bash", hooks: [{ command: "x" }] }] },
      statusLine: { command: "s" },
      outputStyle: "terse",
      enabledPlugins: { "p@m": true },
    },
  },
  { id: "S2", kind: "settings", name: "loose", metadata: {} },
  {
    id: "G",
    kind: "mcp",
    name: "github",
    metadata: { env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } },
  },
  { id: "Q", kind: "mcp", name: "plain", metadata: {} },
];
const find = (id: string) => lib.find((a) => a.id === id);

function item(a: Art) {
  return {
    id: a.id,
    kind: a.kind,
    name: a.name,
    description: a.description ?? null,
    source: { id: "s", kind: "git", label: "addy/skills", branch: null },
    tags: [],
    importedAt: 1n,
    errorCount: 0,
    warningCount: 0,
    fileCount: 1,
    referenceCount: 0,
    resolvedReferences: 0,
    trustScore: a.trust === undefined ? 70 : a.trust,
    tier: null,
    findingCount: a.findings ?? 0,
    bundleSize: a.bundle?.length ?? 0,
    update: null,
    usage: null,
    status: "current",
  };
}

function detail(a: Art) {
  return {
    id: a.id,
    artifact: { kind: a.kind, name: a.name, metadata: a.metadata ?? {} },
    provenance: {
      source: {
        id: "s",
        kind: "git",
        location: "https://github.com/addy/skills",
        branch: null,
        subfolder: null,
        commitSha: null,
      },
      sourcePath: "",
      digest: null,
      importedAt: 1,
      replacedSecretKeys: [],
    },
    files: a.files ?? ["x"],
    audit:
      a.trust === null || a.trust === undefined
        ? null
        : { score: a.trust, tier: "low", findings: [], reach: {} },
    auditedAt: null,
    links: { needs: [], neededBy: [] },
  };
}

function resolve(spec: ProfileSpec) {
  const artifacts: ResolvedItem[] = [];
  const make = (id: string, why: ResolvedItem["why"]): ResolvedItem => {
    const a = find(id);
    return {
      id,
      kind: a?.kind ?? "skill",
      name: a?.name ?? id,
      why,
      manualOnly: spec.manualOnly[id] ?? false,
      fileCount: 1,
    };
  };
  for (const id of spec.items) {
    artifacts.push(make(id, { type: "picked" }));
    for (const m of find(id)?.bundle ?? []) {
      if (!spec.items.includes(m)) {
        artifacts.push(make(m, { type: "pulledBy", artifact: id }));
      }
    }
  }
  if (spec.settings) artifacts.push(make(spec.settings, { type: "settings" }));
  for (const m of spec.mcp) artifacts.push(make(m.artifact, { type: "mcp" }));
  return {
    artifacts,
    missing: spec.items.includes("X") ? ["X"] : [],
    worstTrust: null,
  };
}

function layout(spec: ProfileSpec) {
  const paths = resolve(spec).artifacts.map((a) => ({
    path:
      a.kind === "skill"
        ? `.claude/skills/${a.name}/SKILL.md`
        : `.claude/agents/${a.name}.md`,
    from: { type: "artifact" as const, artifact: a.id },
  }));
  paths.push({
    path: "HARNESS.md",
    from: { type: "generated" as never, generated: "harnessMd" } as never,
  });
  const conflicts =
    spec.items.includes("T") && spec.items.includes("D")
      ? [{ path: ".claude/skills/tdd/SKILL.md", artifacts: ["T", "D"] }]
      : [];
  return { paths, conflicts };
}

interface Opts {
  spec?: ProfileSpec;
  projectCount?: number;
  failGet?: boolean;
  failResolve?: boolean;
  failSave?: boolean;
  failBundle?: boolean;
  failList?: boolean;
}

const base: ProfileSpec = {
  items: ["L"],
  settings: null,
  mcp: [],
  manualOnly: {},
};

function Where() {
  return <p data-testid="where">{useLocation().pathname}</p>;
}

async function setup(path: string, opts: Opts = {}) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, raw) => {
    const args = (raw ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args });
    switch (cmd) {
      case "profile_get":
        if (opts.failGet) throw new Error("no such profile");
        return {
          id: 4,
          name: "Heavy",
          description: "",
          version: 2,
          currentVersion: 2,
          spec: opts.spec ?? base,
          versions: [],
          createdAt: 1,
          updatedAt: 1,
        };
      case "profiles_list":
        return [{ id: 4, projectCount: opts.projectCount ?? 0 }];
      case "library_list": {
        if (opts.failList) throw new Error("library down");
        const filter = args["query"] as {
          filter: { types: string[]; text: string | null };
        };
        const { types, text } = filter.filter;
        const items = lib
          .filter((a) => types.includes(a.kind))
          .filter((a) => !text || a.name.includes(text))
          .map(item);
        return { items, total: items.length, unfilteredTotal: items.length };
      }
      case "library_detail": {
        const a = find(args["id"] as string);
        if (!a) throw new Error("gone");
        return detail(a);
      }
      case "bundle_of":
        if (opts.failBundle) throw new Error("bundle failed");
        return find(args["id"] as string)?.bundle ?? [];
      case "profile_resolve":
        if (opts.failResolve) throw new Error("resolve failed");
        return resolve(args["spec"] as ProfileSpec);
      case "profile_layout":
        return layout(args["spec"] as ProfileSpec);
      case "profile_save":
        if (opts.failSave) throw new Error("disk full");
        return 3;
      case "profile_create":
        return 9;
      case "profile_rename":
        return null;
      default:
        return null;
    }
  });
  const router = createMemoryRouter(
    [
      {
        path: "/",
        element: (
          <>
            <Link to="/elsewhere">Go elsewhere</Link>
            <Where />
            <Outlet />
          </>
        ),
        children: [
          { path: "elsewhere", element: <p>Elsewhere page</p> },
          { path: "profiles/:id", element: <ProfileBuilder /> },
        ],
      },
    ],
    { initialEntries: [path] },
  );
  render(<RouterProvider router={router} />);
  if (!opts.failGet) {
    await screen.findByRole("complementary", { name: "Profile" });
  }
  return { calls, router };
}

const cart = () => screen.getByRole("complementary", { name: "Profile" });
const callsOf = (
  calls: { cmd: string; args: Record<string, unknown> }[],
  cmd: string,
) => calls.filter((c) => c.cmd === cmd);

beforeEach(() => {
  dropDraft("4");
  dropDraft("new");
});

describe("Builder: picker and cart", () => {
  it("lists the library without Settings and MCP, and shows bundle, source, trust", async () => {
    const { calls } = await setup("/profiles/4");
    const row = (
      await screen.findByText("review", { selector: "div" })
    ).closest("li") as HTMLElement;
    expect(within(row).getByText("bundle of 2 · addy/skills")).toBeVisible();
    expect(within(row).getByText("trust 90")).toBeVisible();
    const query = callsOf(calls, "library_list").find(
      (c) => (c.args["query"] as { sort: string }).sort === "sessions",
    )?.args["query"] as {
      filter: { types: string[] };
      sort: string;
      descending: boolean;
    };
    expect(query.filter.types).not.toContain("settings");
    expect(query.filter.types).not.toContain("mcp");
    expect(query.sort).toBe("sessions");
    expect(query.descending).toBe(true);
    expect(
      screen.getByRole("button", { name: "Added review, remove" }),
    ).toBeVisible();
    expect(screen.getByText(/not audited/)).toBeVisible();
  });

  it("filters by text and by type segment", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4");
    await screen.findByText("dup", { selector: "div" });
    await user.type(
      screen.getByRole("textbox", { name: "Search library" }),
      "tdd",
    );
    await waitFor(() =>
      expect(screen.queryByText("dup", { selector: "div" })).toBeNull(),
    );
    await user.click(screen.getByRole("button", { name: "Subagents" }));
    await screen.findByText("No matches.");
    const last = callsOf(calls, "library_list").at(-1)?.args["query"] as {
      filter: { types: string[]; text: string };
    };
    expect(last.filter.types).toEqual(["agent"]);
    expect(last.filter.text).toBe("tdd");
  });

  it("Add pulls the bundle into the cart and the tree at once", async () => {
    const user = userEvent.setup();
    await setup("/profiles/new");
    await user.click(await screen.findByRole("button", { name: "Add review" }));
    await within(cart()).findByText("pulled in by review");
    expect(screen.getByRole("status")).toHaveTextContent(
      "Added review + 1 bundled",
    );
    await waitFor(() =>
      expect(within(cart()).getByText("3 files")).toBeVisible(),
    );
    expect(within(cart()).getByText("1 skill · 1 subagent")).toBeVisible();
    expect(screen.getByText("Unsaved")).toBeVisible();
  });

  it("warns when the added artifact has a low trust score", async () => {
    const user = userEvent.setup();
    await setup("/profiles/new");
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Added tdd. Trust 40: 3 security findings.",
    );
  });

  it("flags a low-trust artifact in the cart and keeps the warning until dismissed", async () => {
    const user = userEvent.setup();
    await setup("/profiles/new");
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    expect(await within(cart()).findByText("trust 40")).toBeVisible();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(7000);
    });
    vi.useRealTimers();
    expect(screen.getByRole("status")).toHaveTextContent("Trust 40");
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows the first rows of a long cart and the rest on Show all", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", {
      spec: { ...base, items: ["L", "T", "D", "R1", "R2", "R3"] },
    });
    await within(cart()).findByText("pulled in by review");
    expect(within(cart()).getByText("7 artifacts")).toBeVisible();
    const list = () => within(cart()).getByRole("list", { name: "Artifacts" });
    expect(within(list()).getAllByRole("listitem")).toHaveLength(5);
    await user.click(
      within(cart()).getByRole("button", { name: "Show all 7" }),
    );
    expect(within(list()).getAllByRole("listitem")).toHaveLength(7);
    await user.click(
      within(cart()).getByRole("button", { name: "Show fewer" }),
    );
    expect(within(list()).getAllByRole("listitem")).toHaveLength(5);
  });

  it("asks before removing a member a picked lead needs, and removes both", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    await within(cart()).findByText("pulled in by review");
    await user.click(
      screen.getByRole("button", { name: "Remove reviewer from the profile" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "reviewer is part of a bundle",
    });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(within(cart()).getByText("pulled in by review")).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Remove reviewer from the profile" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Remove reviewer and review" }),
    );
    await waitFor(() =>
      expect(within(cart()).queryByText("pulled in by review")).toBeNull(),
    );
    expect(within(cart()).getByText(/Nothing yet/)).toBeVisible();
  });

  it("removes a picked lead alone, or with the members only it needs", async () => {
    const user = userEvent.setup();
    const both: ProfileSpec = { ...base, items: ["L", "M"] };
    await setup("/profiles/4", { spec: both });
    await within(cart()).findByText("reviewer");
    await user.click(
      screen.getByRole("button", { name: "Remove review from the profile" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Remove review" });
    expect(within(dialog).getByRole("checkbox")).toBeChecked();
    await user.click(within(dialog).getByRole("checkbox"));
    await user.click(
      within(dialog).getByRole("button", { name: "Remove review" }),
    );
    // Unchecked: the member stays, now as a pick of its own.
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const list = within(cart()).getByRole("list", { name: "Artifacts" });
    await waitFor(() => expect(within(list).queryByText("review")).toBeNull());
    expect(within(list).getByText("reviewer")).toBeVisible();
  });

  it("removes the lead and its members when the box stays checked", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", { spec: { ...base, items: ["L", "M"] } });
    await within(cart()).findByText("reviewer");
    await user.click(
      screen.getByRole("button", { name: "Remove review from the profile" }),
    );
    await user.click(
      await screen.findByRole("button", {
        name: "Remove review and its members",
      }),
    );
    await within(cart()).findByText(/Nothing yet/);
  });

  it("removes a plain item without asking", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", { spec: { ...base, items: ["T"] } });
    await user.click(
      await screen.findByRole("button", { name: "Added tdd, remove" }),
    );
    await within(cart()).findByText(/Nothing yet/);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows a failed removal check and a failed library", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", { failBundle: true, failList: true });
    expect(
      await screen.findByText(
        /The library did not load: Error: library down|The library did not load/,
      ),
    ).toBeVisible();
    await user.click(
      await screen.findByRole("button", {
        name: "Remove review from the profile",
      }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "bundle failed",
    );
  });

  it("shows resolve errors, missing artifacts and conflicts in red", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", { spec: { ...base, items: ["T", "D", "X"] } });
    await within(cart()).findAllByRole("button", { name: /SKILL\.md/ });
    expect(
      await within(cart()).findByText(/No longer in the library: X/),
    ).toBeVisible();
    expect(within(cart()).getByText(/1 path is claimed twice/)).toBeVisible();
    expect(
      within(cart()).getByText(/is claimed by tdd and dup\. Remove one\./),
    ).toBeVisible();
    expect(
      within(cart()).getByRole("button", { name: "Remove tdd" }),
    ).toBeVisible();
    const flagged = within(cart()).getAllByText("conflict");
    expect(flagged).toHaveLength(1);
    await user.click(
      within(cart()).getByRole("button", { name: "Remove dup" }),
    );
    await waitFor(() =>
      expect(
        within(cart()).queryByRole("button", { name: "Remove dup" }),
      ).toBeNull(),
    );
  });

  it("reports a resolve failure", async () => {
    await setup("/profiles/4", { failResolve: true });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "resolve failed",
    );
  });

  it("shows the not-found state", async () => {
    await setup("/profiles/4", { failGet: true });
    expect(
      await screen.findByRole("heading", { name: "Profile not found" }),
    ).toBeVisible();
    expect(screen.queryByText(/no such profile/)).toBeNull();
    expect(screen.getByText(/no longer in your library/)).toBeVisible();
  });
});

describe("Builder: Settings template", () => {
  it("lists templates with their summary and picks exactly one or none", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4");
    expect(
      await within(cart()).findByText(
        "deny 2 · ask 1 · allow 3 · 1 hook · status line · style: terse · 1 plugin",
      ),
    ).toBeVisible();
    expect(
      within(cart()).getByText("deny 0 · ask 0 · allow 0 · 0 hooks"),
    ).toBeVisible();
    const strict = within(cart()).getByRole("radio", { name: /strict/ });
    expect(within(cart()).getByRole("radio", { name: /None/ })).toBeChecked();
    await user.click(strict);
    expect(strict).toBeChecked();
    await user.click(within(cart()).getByRole("radio", { name: /loose/ }));
    expect(strict).not.toBeChecked();
    await user.click(within(cart()).getByRole("radio", { name: /None/ }));
    await user.click(strict);
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(callsOf(calls, "profile_save")).toHaveLength(1));
    expect(
      (callsOf(calls, "profile_save")[0]?.args["spec"] as ProfileSpec).settings,
    ).toBe("S");
    expect(
      within(cart()).getByRole("link", { name: "Manage" }),
    ).toHaveAttribute("href", "/library?type=settings");
  });

  it("shows the template's settings.json and closes it", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    await user.click(
      await within(cart()).findByRole("button", {
        name: "View strict settings.json",
      }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "strict: settings.json",
    });
    expect(within(dialog).getByLabelText("settings.json")).toHaveTextContent(
      '"outputStyle": "terse"',
    );
    expect(
      within(dialog).getByText(
        /Hook scripts are copied with it: hooks\/guard\.sh/,
      ),
    ).toBeVisible();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(
      within(cart()).getByRole("button", { name: "View strict settings.json" }),
    );
    await user.click(await screen.findByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});

describe("Builder: MCP servers", () => {
  it("adds a server with placeholder env rows and removes it", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    const select = await within(cart()).findByRole("combobox", {
      name: "Add MCP server",
    });
    await screen.findByRole("option", { name: "github" });
    await user.selectOptions(select, "G");
    const input = await within(cart()).findByLabelText("GITHUB_TOKEN");
    expect(input).toHaveValue("${GITHUB_TOKEN}");
    // A server without env keys gets a generic one.
    await user.selectOptions(
      within(cart()).getByRole("combobox", { name: "Add MCP server" }),
      "Q",
    );
    expect(await within(cart()).findByLabelText("API_TOKEN")).toHaveValue(
      "${API_TOKEN}",
    );
    await user.click(
      within(cart()).getByRole("button", { name: "Remove plain server" }),
    );
    expect(within(cart()).queryByLabelText("API_TOKEN")).toBeNull();
    await user.click(
      within(cart()).getByRole("button", { name: "Remove github server" }),
    );
    expect(within(cart()).getByText("No servers.")).toBeVisible();
  });

  it("keeps the placeholder and warns when the value is not one", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4", {
      spec: {
        ...base,
        mcp: [{ artifact: "G", env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } }],
      },
    });
    const input = await within(cart()).findByLabelText("GITHUB_TOKEN");
    const secret = ["gh", "p_", "x".repeat(20)].join("");
    await user.clear(input);
    await user.type(input, secret);
    await user.tab();
    const alert = await within(cart()).findByRole("alert");
    expect(alert).toHaveTextContent(
      "That looked like a real secret, so it wasn't saved.",
    );
    expect(alert).not.toHaveTextContent(secret);
    expect(input).toHaveValue("${GITHUB_TOKEN}");
    expect(document.body.textContent).not.toContain(secret);
    expect(within(cart()).getByRole("button", { name: "Save" })).toBeDisabled();
    // A real placeholder goes through, with Enter or on blur.
    await user.clear(input);
    await user.type(input, "$" + "{{GH_TOKEN}{Enter}");
    await waitFor(() => expect(input).toHaveValue("${GH_TOKEN}"));
    expect(within(cart()).queryByRole("alert")).toBeNull();
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(callsOf(calls, "profile_save")).toHaveLength(1));
    const spec = callsOf(calls, "profile_save")[0]?.args["spec"] as ProfileSpec;
    expect(spec.mcp[0]?.env).toEqual({ GITHUB_TOKEN: "${GH_TOKEN}" });
  });

  it("keeps an unchanged placeholder as it is", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", {
      spec: {
        ...base,
        mcp: [{ artifact: "G", env: { GITHUB_TOKEN: "${GITHUB_TOKEN}" } }],
      },
    });
    const input = await within(cart()).findByLabelText("GITHUB_TOKEN");
    await user.click(input);
    await user.tab();
    expect(within(cart()).queryByRole("alert")).toBeNull();
    expect(within(cart()).getByRole("button", { name: "Save" })).toBeDisabled();
  });
});

describe("Builder: tree and Manual only", () => {
  it("shows where a file comes from and writes the Manual only override", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4");
    const file = await within(cart()).findByRole("button", {
      name: "SKILL.md",
    });
    await user.click(file);
    expect(
      await within(cart()).findByText("From review · addy/skills"),
    ).toBeVisible();
    const toggle = within(cart()).getByRole("switch", {
      name: "Manual only for review",
    });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    await user.click(toggle);
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    expect(within(cart()).getByText("manual only")).toBeVisible();
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(callsOf(calls, "profile_save")).toHaveLength(1));
    expect(
      (callsOf(calls, "profile_save")[0]?.args["spec"] as ProfileSpec)
        .manualOnly,
    ).toEqual({ L: true });
    // A second click on the file closes the info.
    await user.click(file);
    expect(within(cart()).queryByText("From review · addy/skills")).toBeNull();
  });

  it("says a generated file is generated, and has no toggle on non-skills", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    await user.click(
      await within(cart()).findByRole("button", { name: "HARNESS.md" }),
    );
    expect(within(cart()).getByText("Generated by Harneo")).toBeVisible();
    expect(within(cart()).queryByRole("switch")).toBeNull();
    await user.click(
      within(cart()).getByRole("button", { name: "reviewer.md" }),
    );
    expect(within(cart()).queryByRole("switch")).toBeNull();
  });
});

describe("Builder: save", () => {
  it("shows the next version, saves it and reports it", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4", { projectCount: 2 });
    expect(await within(cart()).findByText("v2")).toBeVisible();
    expect(within(cart()).getByRole("button", { name: "Save" })).toBeDisabled();
    expect(
      within(cart()).getByText(
        /Used by 2 projects\. Saving creates v3; projects keep v2/,
      ),
    ).toBeVisible();
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    expect(await within(cart()).findByText("v2 → v3 on save")).toBeVisible();
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved Heavy v3")).toBeVisible();
    expect(within(cart()).getByText("v3")).toBeVisible();
    expect(screen.queryByText("Unsaved")).toBeNull();
    expect(callsOf(calls, "profile_rename")).toHaveLength(0);
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText("Saved Heavy v3")).toBeNull();
  });

  it("renames without a new version", async () => {
    const user = userEvent.setup();
    const { calls } = await setup("/profiles/4");
    const name = await within(cart()).findByLabelText("Profile name");
    await user.clear(name);
    await user.type(name, "Heavier");
    expect(within(cart()).getByText("v2")).toBeVisible();
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    expect(await screen.findByText("Saved Heavier v2")).toBeVisible();
    expect(callsOf(calls, "profile_save")).toHaveLength(0);
    expect(callsOf(calls, "profile_rename")[0]?.args).toEqual({
      id: 4,
      name: "Heavier",
    });
  });

  it("discards the changes", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    await user.click(within(cart()).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(screen.queryByText("Unsaved")).toBeNull());
    // The picker follows once the cart has resolved the reverted spec.
    expect(
      await screen.findByRole("button", { name: "Add tdd" }),
    ).toBeVisible();
  });

  it("shows a failed save and keeps the changes", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4", { failSave: true });
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText(/Could not save: Error: disk full/),
    ).toBeVisible();
    expect(screen.getByText("Unsaved")).toBeVisible();
  });

  it("creates a new profile on first save, named by default, and opens it", async () => {
    const user = userEvent.setup();
    const { calls, router } = await setup("/profiles/new");
    expect(await within(cart()).findByText("new · v1")).toBeVisible();
    expect(within(cart()).getByRole("button", { name: "Save" })).toBeDisabled();
    await user.type(within(cart()).getByLabelText("Profile name"), "x");
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("Add at least one artifact before saving."),
    ).toBeVisible();
    await user.clear(within(cart()).getByLabelText("Profile name"));
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    await user.click(within(cart()).getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(screen.getByTestId("where")).toHaveTextContent("/profiles/9"),
    );
    expect(router.state.location.state).toEqual({
      notice: { text: "Saved Untitled profile v1", tone: "ok", sticky: true },
    });
    const create = callsOf(calls, "profile_create")[0]?.args;
    expect(create?.["name"]).toBe("Untitled profile");
    expect((create?.["spec"] as ProfileSpec).items).toEqual(["T"]);
  });
});

describe("Builder: leaving with unsaved changes", () => {
  it("asks first, stays on Cancel, leaves on Leave and keeps the draft", async () => {
    const user = userEvent.setup();
    const { router } = await setup("/profiles/4");
    await user.click(await screen.findByRole("button", { name: "Add tdd" }));
    await user.click(screen.getByRole("link", { name: "Go elsewhere" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Leave without saving?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.getByTestId("where")).toHaveTextContent("/profiles/4");
    await user.click(screen.getByRole("link", { name: "Go elsewhere" }));
    await user.click(await screen.findByRole("button", { name: "Leave" }));
    expect(await screen.findByText("Elsewhere page")).toBeVisible();
    // Coming back brings the draft with it.
    void router.navigate("/profiles/4");
    expect(await screen.findByText("Unsaved")).toBeVisible();
    await screen.findByRole("complementary", { name: "Profile" });
    const list = await within(cart()).findByRole("list", { name: "Artifacts" });
    expect(await within(list).findByText("tdd")).toBeVisible();
  });

  it("does not ask when nothing changed", async () => {
    const user = userEvent.setup();
    await setup("/profiles/4");
    await within(cart()).findByText("v2");
    await user.click(screen.getByRole("link", { name: "Go elsewhere" }));
    expect(await screen.findByText("Elsewhere page")).toBeVisible();
  });
});
