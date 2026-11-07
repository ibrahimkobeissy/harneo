import { mockIPC } from "@tauri-apps/api/mocks";
import { describe, expect, it } from "vitest";
import {
  artifactSetManualOnly,
  profileClone,
  profileCreate,
  profileSetCategory,
  templatesList,
  profileDelete,
  profileDescribe,
  profileFromCollection,
  profileGet,
  profileImportCommit,
  profileLayout,
  profileRename,
  profileResolve,
  profileSave,
} from "./ipc";

const spec = { items: ["a"], settings: null, mcp: [], manualOnly: {} };

describe("profile wrappers", () => {
  it("send ids as numbers with the command's argument names", async () => {
    const calls: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [
        "profile_create",
        "profile_clone",
        "profile_from_collection",
      ].includes(cmd)
        ? 5
        : null;
    });
    expect(await profileCreate("A", "d", spec)).toBe(5n);
    await profileCreate("B", "", spec, "custom", "my steps");
    await profileSetCategory(5n, "custom", "note");
    await profileSetCategory(5n, "video");
    await templatesList();
    expect(await profileClone(5n, "B")).toBe(5n);
    await profileGet(5n);
    await profileGet(5n, 2);
    await profileSave(5n, spec);
    await profileRename(5n, "C");
    await profileDescribe(5n, "d2");
    await profileDelete(5n);
    await profileFromCollection(3n, "From");
    await profileResolve(spec);
    await profileLayout(spec);
    await profileImportCommit("tok");
    await profileImportCommit("tok", "N");
    await artifactSetManualOnly("a", true);
    expect(calls).toEqual([
      [
        "profile_create",
        {
          name: "A",
          description: "d",
          spec,
          category: "software",
          categoryNote: null,
        },
      ],
      [
        "profile_create",
        {
          name: "B",
          description: "",
          spec,
          category: "custom",
          categoryNote: "my steps",
        },
      ],
      ["profile_set_category", { id: 5, category: "custom", note: "note" }],
      ["profile_set_category", { id: 5, category: "video", note: null }],
      ["templates_list", {}],
      ["profile_clone", { id: 5, name: "B" }],
      ["profile_get", { id: 5, version: null }],
      ["profile_get", { id: 5, version: 2 }],
      ["profile_save", { id: 5, spec }],
      ["profile_rename", { id: 5, name: "C" }],
      ["profile_describe", { id: 5, description: "d2" }],
      ["profile_delete", { id: 5 }],
      ["profile_from_collection", { collectionId: 3, name: "From" }],
      ["profile_resolve", { spec }],
      ["profile_layout", { spec }],
      ["profile_import_commit", { token: "tok", name: null }],
      ["profile_import_commit", { token: "tok", name: "N" }],
      ["artifact_set_manual_only", { id: "a", manualOnly: true }],
    ]);
  });
});
