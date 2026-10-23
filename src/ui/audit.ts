import type { AuditSeverity } from "../bindings/AuditSeverity";
import type { Tier } from "../bindings/Tier";
import "./audit.css";

/**
 * The one source of the audit vocabulary: tier and severity order, labels and colours, and the
 * rule catalogue. Colours live in audit.css, keyed by `data-tier` and `data-sev`.
 */

export interface TierInfo {
  tier: Tier;
  /** Display label. The id `review` reads "Needs review" so it never clashes with the import step. */
  label: string;
  /** Lowest and highest score in the tier. */
  lo: number;
  hi: number;
}

/** The four bands of a trust score, best first. */
export const tiers: readonly TierInfo[] = [
  { tier: "trusted", label: "Trusted", lo: 80, hi: 100 },
  { tier: "review", label: "Needs review", lo: 60, hi: 79 },
  { tier: "risky", label: "Risky", lo: 40, hi: 59 },
  { tier: "dangerous", label: "Dangerous", lo: 0, hi: 39 },
];

export const tierOrder: readonly Tier[] = tiers.map((t) => t.tier);

export const tierLabels: Record<Tier, string> = {
  trusted: "Trusted",
  review: "Needs review",
  risky: "Risky",
  dangerous: "Dangerous",
};

/** Most serious first. */
export const severityOrder: readonly AuditSeverity[] = [
  "critical",
  "high",
  "medium",
  "low",
];

/** Higher is more serious. */
export const severityRank: Record<AuditSeverity, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
};

export const severityLabels: Record<AuditSeverity, string> = {
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
};

/** The Badge tone of each severity (critical also gets the solid SeverityBadge). */
export const severityTone: Record<AuditSeverity, "bad" | "warn" | "neutral"> = {
  critical: "bad",
  high: "bad",
  medium: "warn",
  low: "neutral",
};

export interface RuleInfo {
  title: string;
  explanation: string;
}

/** Every rule the core registers (`audit::rules()`): a plain title and a one-line explanation. */
export const ruleCatalogue: Record<string, RuleInfo> = {
  "prompt-injection": {
    title: "Tries to override the agent's instructions",
    explanation:
      "Text that tells the model to ignore its rules, hide its actions or obey someone else.",
  },
  "hidden-unicode": {
    title: "Hides text from reviewers",
    explanation:
      "Invisible or direction-changing characters can conceal instructions in plain sight.",
  },
  "plaintext-secrets": {
    title: "Contains a secret in plain text",
    explanation: "An API key, token or password is written into the file.",
  },
  "credential-theft": {
    title: "Reads credentials",
    explanation:
      "Touches key stores, tokens or environment variables that hold credentials.",
  },
  rce: {
    title: "Downloads and runs code",
    explanation:
      "Pipes a download into a shell or evaluates fetched code, so whoever controls the source controls your machine.",
  },
  "dangerous-commands": {
    title: "Runs destructive commands",
    explanation:
      "Commands that delete files, loosen permissions or overwrite disks.",
  },
  "safety-bypass": {
    title: "Switches off safety checks",
    explanation: "Skips permission prompts, hooks or verification.",
  },
  "broad-permissions": {
    title: "Grants wide permissions",
    explanation:
      "Allows tools, paths or commands far beyond what the artifact needs.",
  },
  "mcp-command-injection": {
    title: "Builds a server command unsafely",
    explanation:
      "The MCP server command runs through a shell or is shaped by untrusted text.",
  },
  "supply-chain": {
    title: "Fetches code without a pin",
    explanation:
      "Installs or enables a package, plugin or server without a fixed version.",
  },
  "permission-combo": {
    title: "Risky mix of permissions",
    explanation:
      "Reaches the network and also the environment or the shell, so data can leave or code can come in.",
  },
};

/** The catalogue entry for a rule id; an id the UI does not know yet shows as itself. */
export function ruleInfo(rule: string): RuleInfo {
  return ruleCatalogue[rule] ?? { title: rule, explanation: "" };
}

/** Explains the `*` that reach lists use. */
export const anyNote = "* means any: every host, command or file.";

/** True when a reach list contains the any marker. */
export function hasAny(list: readonly string[]): boolean {
  return list.includes("*");
}
