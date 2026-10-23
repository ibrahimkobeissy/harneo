import { describe, expect, it } from "vitest";
import { localDay, relativeDay } from "./dates";

describe("localDay", () => {
  it("gives the local calendar day of a timestamp", () => {
    const at = new Date(2026, 9, 24, 0, 30); // 00:30 local, the previous day in UTC east of Greenwich
    expect(localDay(at.toISOString())).toBe("2026-10-24");
    expect(relativeDay(localDay(at.toISOString()), at)).toBe("today");
  });

  it("returns unparseable input as is", () => {
    expect(localDay("not a date")).toBe("not a date");
  });
});
