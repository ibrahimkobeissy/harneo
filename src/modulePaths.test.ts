import { describe, expect, it } from "vitest";

// Windows and macOS file systems are case-insensitive by default: there `./Tooltip` can resolve to
// `tooltip.ts`, so two modules whose paths differ only in casing break the build.
describe("module paths", () => {
  it("never differ only in casing", () => {
    const paths = Object.keys(import.meta.glob("/src/**/*.{ts,tsx}"));
    const byKey = new Map<string, string[]>();
    for (const path of paths) {
      const key = path.replace(/\.tsx?$/, "").toLowerCase();
      byKey.set(key, [...(byKey.get(key) ?? []), path]);
    }
    const clashes = [...byKey.values()].filter((group) => group.length > 1);
    expect(clashes).toEqual([]);
  });
});
