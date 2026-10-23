import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, expect, it } from "vitest";
import { projectsRemoved } from "./ipc";

afterEach(() => clearMocks());

it("projectsRemoved asks for the removed projects", async () => {
  const calls: string[] = [];
  mockIPC((cmd) => {
    calls.push(cmd);
    return [];
  });
  expect(await projectsRemoved()).toEqual([]);
  expect(calls).toEqual(["projects_removed"]);
});
