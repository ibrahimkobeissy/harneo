import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import {
  backupDelete,
  backupRestore,
  projectBackups,
  removalPlan,
  switchPlan,
} from "./ipc";

function record(result: unknown) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return result;
  });
  return calls;
}

describe("switch, removal and backup wrappers", () => {
  it("send ids as numbers with the command's argument names", async () => {
    const calls = record("r");
    expect(await switchPlan(4n, 3n)).toBe("r");
    await switchPlan(4n, 3n, 2);
    expect(await removalPlan(4n)).toBe("r");
    expect(await projectBackups(4n)).toBe("r");
    expect(await backupRestore(9n)).toBe("r");
    await backupDelete(9n);
    expect(calls).toEqual([
      {
        cmd: "switch_plan",
        args: { projectId: 4, profileId: 3, version: null },
      },
      {
        cmd: "switch_plan",
        args: { projectId: 4, profileId: 3, version: 2 },
      },
      { cmd: "removal_plan", args: { projectId: 4 } },
      { cmd: "project_backups", args: { id: 4 } },
      { cmd: "backup_restore", args: { backupId: 9 } },
      { cmd: "backup_delete", args: { backupId: 9 } },
    ]);
  });

  it("reject with the backend's message", async () => {
    mockIPC(() => {
      throw "project 7 does not exist";
    });
    await expect(removalPlan(7n)).rejects.toBe("project 7 does not exist");
    await expect(backupDelete(7n)).rejects.toBe("project 7 does not exist");
  });
});
