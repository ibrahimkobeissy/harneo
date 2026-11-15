import { describe, expect, it } from "vitest";
import { draftFor, dropDraft, keepDraft } from "./drafts";

const spec = { items: ["a"], settings: null, mcp: [], manualOnly: {} };

describe("drafts", () => {
  it("opens a draft only on the profile it was kept for", () => {
    keepDraft("5", { base: "A", name: "Old", spec });
    expect(draftFor("5", "A")?.name).toBe("Old");
    // A new profile that reuses id 5 is not the one the draft belongs to.
    expect(draftFor("5", "B")).toBeUndefined();
    dropDraft("5");
    expect(draftFor("5", "A")).toBeUndefined();
  });
});
