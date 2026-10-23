import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { describe, expect, it } from "vitest";
import type { Detection } from "../../bindings/Detection";
import { item, page } from "../../test/library";
import { LibraryScreen } from ".";

const detection: Detection = {
  links: [
    {
      from: "id-alpha",
      to: "id-beta",
      reason: "Alpha hands off to beta.",
      confidence: "high",
    },
    {
      from: "id-alpha",
      to: "id-gamma",
      reason: "Alpha hands off to beta.",
      confidence: "high",
    },
    {
      from: "id-delta",
      to: "id-beta",
      reason: "Delta calls beta.",
      confidence: "medium",
    },
  ],
  tags: [],
};

interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

function setup(opts: {
  path?: string;
  cliFound?: boolean;
  detect?: () => Detection;
}) {
  const calls: Call[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    const id = (args as { id?: string } | undefined)?.id ?? "";
    switch (cmd) {
      case "library_list":
        return page(
          ["alpha", "beta", "gamma", "delta"].map((n) => item(n, {}, "agent")),
        );
      case "library_detail":
        return {
          id,
          artifact: {
            kind: "agent",
            name: id.replace("id-", ""),
            description: "",
            path: "p",
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
          provenance: {
            source: {
              id: "s1",
              kind: "local",
              location: "/x",
              branch: null,
              subfolder: null,
              commitSha: null,
            },
            sourcePath: "p",
            digest: null,
            importedAt: 1_700_000_000n,
            replacedSecretKeys: [],
          },
          files: [],
        };
      case "links_of":
        return { needs: [], neededBy: [] };
      case "claude_cli_status":
        return opts.cliFound === false
          ? { found: false, path: null, version: null }
          : { found: true, path: "/bin/claude", version: "2.0" };
      case "bundles_detect_library":
        return opts.detect ? opts.detect() : detection;
      case "tags_list":
      case "artifact_tags":
        return [];
      default:
        return undefined;
    }
  });
  render(
    <MemoryRouter initialEntries={[opts.path ?? "/library"]}>
      <Routes>
        <Route path="/library" element={<LibraryScreen />} />
      </Routes>
    </MemoryRouter>,
  );
  return calls;
}

describe("Find bundles with Claude", () => {
  it("runs on the whole library from the toolbar menu and adds the accepted links", async () => {
    const user = userEvent.setup();
    const calls = setup({});
    await user.click(
      await screen.findByRole("button", { name: "More library actions" }),
    );
    await user.click(
      screen.getByRole("menuitem", { name: /Find bundles with Claude/ }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Find bundles with Claude",
    });
    expect(await within(dialog).findByText("+ 2")).toBeVisible();
    expect(calls.find((c) => c.cmd === "bundles_detect_library")?.args).toEqual(
      { ids: null },
    );
    // Refuse the delta group; keep alpha's.
    await user.click(
      within(dialog).getByRole("switch", { name: "Accept bundle of delta" }),
    );
    await user.click(
      within(dialog).getByRole("button", { name: "Add 2 links" }),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Find bundles with Claude" }),
      ).toBeNull(),
    );
    const adds = calls.filter((c) => c.cmd === "link_add").map((c) => c.args);
    expect(adds).toEqual([
      {
        from: "id-alpha",
        to: "id-beta",
        origin: "ai",
        reason: "Alpha hands off to beta.",
      },
      {
        from: "id-alpha",
        to: "id-gamma",
        origin: "ai",
        reason: "Alpha hands off to beta.",
      },
    ]);
    expect(
      calls.filter((c) => c.cmd === "link_reject").map((c) => c.args),
    ).toEqual([{ from: "id-delta", to: "id-beta" }]);
  });

  it("runs on one artifact from its detail panel", async () => {
    const user = userEvent.setup();
    const calls = setup({ path: "/library?open=id-alpha" });
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: "Find bundles" }),
    );
    expect(await screen.findByText("+ 2")).toBeVisible();
    expect(calls.find((c) => c.cmd === "bundles_detect_library")?.args).toEqual(
      { ids: ["id-alpha"] },
    );
  });

  it("says when there is nothing to add", async () => {
    const user = userEvent.setup();
    setup({
      path: "/library?open=id-alpha",
      detect: () => ({ links: [], tags: [] }),
    });
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: "Find bundles" }),
    );
    expect(await screen.findByText(/No new bundles found/)).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Nothing to add" }),
    ).toBeDisabled();
  });

  it("reports a missing CLI", async () => {
    const user = userEvent.setup();
    const calls = setup({ path: "/library?open=id-alpha", cliFound: false });
    const panel = await screen.findByRole("dialog", { name: "alpha details" });
    await user.click(
      await within(panel).findByRole("button", { name: "Find bundles" }),
    );
    expect(await screen.findByText("Claude CLI not found.")).toBeVisible();
    expect(calls.some((c) => c.cmd === "bundles_detect_library")).toBe(false);
  });
});
