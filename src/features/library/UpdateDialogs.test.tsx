import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { FileDiff } from "../../bindings/FileDiff";
import type { UpdateReport } from "../../bindings/UpdateReport";
import { item, page } from "../../test/library";
import {
  LocalEditDialog,
  UpdateAllDialog,
  UpdateDialog,
  versionStep,
} from "./UpdateDialogs";

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

const diff: FileDiff[] = [{ path: "a/SKILL.md", status: "removed", hunks: [] }];

describe("versionStep", () => {
  it("describes both versions, only the new one, or none", () => {
    expect(versionStep("1", "2")).toBe("1 → 2");
    expect(versionStep(null, "2")).toBe("→ 2");
    expect(versionStep("1", null)).toBeNull();
  });
});

describe("UpdateDialog", () => {
  function open(onUpdated = vi.fn()) {
    render(
      <UpdateDialog
        id="id-a"
        name="a"
        fromVersion={null}
        toVersion="2"
        onClose={() => {}}
        onUpdated={onUpdated}
      />,
    );
    return onUpdated;
  }

  it("shows the error when the comparison fails", async () => {
    mockIPC((cmd) => {
      if (cmd === "update_diff") throw "source offline";
      return undefined;
    });
    open();
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "source offline",
    );
  });

  it("shows the message of a failed update and does not report it as updated", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "update_diff") return diff;
      if (cmd === "update_apply")
        return report("id-a", "failed", { message: "disk full" });
      return undefined;
    });
    const onUpdated = open();
    await screen.findByText("a/SKILL.md");
    await user.click(
      screen.getByRole("button", { name: "Update library copy" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(onUpdated).not.toHaveBeenCalled();
  });

  it("shows the error when applying throws", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "update_diff") return diff;
      if (cmd === "update_apply") throw "apply broke";
      return undefined;
    });
    open();
    await screen.findByText("a/SKILL.md");
    await user.click(
      screen.getByRole("button", { name: "Update library copy" }),
    );
    expect(await screen.findByText(/apply broke/)).toBeInTheDocument();
  });
});

describe("LocalEditDialog", () => {
  it("states the backup location and what a removed file does before confirming", async () => {
    mockIPC((cmd) => (cmd === "update_diff" ? diff : undefined));
    render(
      <LocalEditDialog
        id="id-a"
        name="a"
        toVersion="2"
        onClose={() => {}}
        onUpdated={() => {}}
      />,
    );
    expect(
      await screen.findByText("Take upstream deletes this file."),
    ).toBeInTheDocument();
    expect(
      screen.getByText("library/.backups/id-a/<date>"),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { level: 3, name: /a\/SKILL\.md/ }),
    ).toBeInTheDocument();
  });

  it("shows the error when applying throws, and a failed report's message", async () => {
    const user = userEvent.setup();
    let fail: "throw" | "report" = "throw";
    mockIPC((cmd) => {
      if (cmd === "update_diff") return diff;
      if (cmd === "update_apply") {
        if (fail === "throw") throw "apply broke";
        return report("id-a", "failed", { message: "nope" });
      }
      return undefined;
    });
    render(
      <LocalEditDialog
        id="id-a"
        name="a"
        toVersion="2"
        onClose={() => {}}
        onUpdated={() => {}}
      />,
    );
    await screen.findByText("a/SKILL.md");
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    expect(await screen.findByText(/apply broke/)).toBeInTheDocument();
    fail = "report";
    await user.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(screen.getByText("nope")).toBeInTheDocument());
  });
});

describe("UpdateAllDialog", () => {
  const rows = [item("a", { update: { toVersion: "2", localEdits: false } })];

  it("shows the error when listing the outdated artifacts fails", async () => {
    mockIPC((cmd) => {
      if (cmd === "library_list") throw "list broke";
      return undefined;
    });
    render(<UpdateAllDialog onClose={() => {}} onUpdated={() => {}} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("list broke");
  });

  it("lists failures per artifact", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "library_list") return page(rows);
      if (cmd === "update_all")
        return [report("id-a", "failed", { message: "bad ref" })];
      return undefined;
    });
    render(<UpdateAllDialog onClose={() => {}} onUpdated={() => {}} />);
    const dialog = await screen.findByRole("dialog", { name: "Update all" });
    await user.click(
      await within(dialog).findByRole("button", { name: "Update 1" }),
    );
    expect(await within(dialog).findByText("a: bad ref")).toBeInTheDocument();
    expect(
      within(dialog).getByText("Updated 0 artifacts."),
    ).toBeInTheDocument();
  });

  it("shows the error when the call throws", async () => {
    const user = userEvent.setup();
    mockIPC((cmd) => {
      if (cmd === "library_list") return page(rows);
      if (cmd === "update_all") throw "all broke";
      return undefined;
    });
    render(<UpdateAllDialog onClose={() => {}} onUpdated={() => {}} />);
    const dialog = await screen.findByRole("dialog", { name: "Update all" });
    await user.click(
      await within(dialog).findByRole("button", { name: "Update 1" }),
    );
    expect(await within(dialog).findByText(/all broke/)).toBeInTheDocument();
  });

  it("keeps the actions outside the scrolling list for 30 rows, and shortens the edited names", async () => {
    const many = Array.from({ length: 30 }, (_, n) =>
      item(`r${n}`, {
        update: { toVersion: n % 2 ? "2" : null, localEdits: false },
      }),
    );
    const edited = Array.from({ length: 8 }, (_, n) =>
      item(`e${n}`, { update: { toVersion: "2", localEdits: true } }),
    );
    mockIPC((cmd) =>
      cmd === "library_list" ? page([...many, ...edited]) : undefined,
    );
    render(<UpdateAllDialog onClose={() => {}} onUpdated={() => {}} />);
    const dialog = await screen.findByRole("dialog", { name: "Update all" });
    const update = await within(dialog).findByRole("button", {
      name: "Update 30",
    });
    const list = within(dialog).getAllByRole("list")[0] as HTMLElement;
    const scroll = list.parentElement as HTMLElement;
    expect(scroll.contains(update)).toBe(false);
    expect(scroll.contains(within(dialog).getByRole("note"))).toBe(true);
    expect(within(dialog).getByRole("note")).toHaveTextContent("and 3 more");
    expect(within(dialog).getAllByText("changed")).toHaveLength(15);
  });
});
