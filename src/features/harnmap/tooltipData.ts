import type { Fact } from "../../bindings/Fact";

export type Source = "files" | "claude";

/**
 * Where a tooltip line comes from. Role is always Claude's reading. When and Hands off are
 * tagged `files` when the artifact has a fact of a kind that backs them, `claude` otherwise:
 * - Hands off: `bundle`, `skills`, `allowedMcp` (who it hands work to).
 * - When: `manualOnly`, `rulePaths`, `hook`, `alwaysLoaded`, `outputStyle` (when it applies).
 */
export function sourceOf(
  field: "role" | "when" | "handsOff",
  facts: Fact[],
): Source {
  if (field === "role") return "claude";
  const backing =
    field === "handsOff"
      ? ["bundle", "skills", "allowedMcp"]
      : ["manualOnly", "rulePaths", "hook", "alwaysLoaded", "outputStyle"];
  return facts.some((f) => backing.includes(f.kind.type)) ? "files" : "claude";
}
