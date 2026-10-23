import { describe, expect, it } from "vitest";
import { ruleCatalogue, ruleInfo, tierLabels, tierOrder, tiers } from "./audit";

describe("audit vocabulary", () => {
  it("documents every rule the core registers", () => {
    expect(Object.keys(ruleCatalogue).sort()).toEqual(
      [
        "broad-permissions",
        "credential-theft",
        "dangerous-commands",
        "hidden-unicode",
        "mcp-command-injection",
        "permission-combo",
        "plaintext-secrets",
        "prompt-injection",
        "rce",
        "safety-bypass",
        "supply-chain",
      ].sort(),
    );
    expect(ruleInfo("rce").title).toBe("Downloads and runs code");
    expect(ruleInfo("unknown-rule")).toEqual({
      title: "unknown-rule",
      explanation: "",
    });
  });

  it("calls the review tier Needs review, best first", () => {
    expect(tierLabels.review).toBe("Needs review");
    expect(tierOrder).toEqual(["trusted", "review", "risky", "dangerous"]);
    expect(tiers.map((t) => t.label)).toEqual(
      tierOrder.map((t) => tierLabels[t]),
    );
  });
});
