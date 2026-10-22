import { describe, expect, it } from "vitest";
import { defaultTagColor, sha256 } from "./tagColor";

const hex = (b: Uint8Array) =>
  [...b].map((x) => x.toString(16).padStart(2, "0")).join("");

describe("tag colors", () => {
  it("hashes like SHA-256", () => {
    expect(hex(sha256(new Uint8Array()))).toBe(
      "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
    expect(hex(sha256(new TextEncoder().encode("abc")))).toBe(
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    const long = new TextEncoder().encode("a".repeat(100));
    expect(hex(sha256(long))).toBe(
      "2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e",
    );
  });

  it("is case-insensitive and picks from the palette", () => {
    expect(defaultTagColor("Review")).toBe(defaultTagColor("review"));
    // sha256("abc")[0] = 0xba = 186; 186 % 7 = 4
    expect(defaultTagColor("abc")).toBe("fuchsia");
  });
});
