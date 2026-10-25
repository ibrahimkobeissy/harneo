import { PackageOpen, TriangleAlert } from "lucide-react";
import type { Plan } from "../../bindings/Plan";
import { Callout, decodeText } from "../../ui";

/** HARNESS.md carries this sentence whenever the profile holds a plugin. */
const MARK = "Plugins are not copied.";

/** True when the plan's HARNESS.md lists plugins, or one cannot be enabled. */
export function hasPlugins(plan: Plan): boolean {
  if (plan.warnings.length > 0) return true;
  return plan.writes.some(
    (w) =>
      w.path === "HARNESS.md" &&
      w.bytes !== null &&
      decodeText(w.bytes)?.includes(MARK),
  );
}

/**
 * Why a profile's plugins have no files in the plan, and which of them cannot be enabled.
 * Nothing when the profile holds no plugin.
 */
export function PluginNotice({ plan }: { plan: Plan }) {
  if (!hasPlugins(plan)) return null;
  return (
    <>
      {plan.warnings.length > 0 && (
        <Callout tone="warn" icon={TriangleAlert}>
          <p>
            <b>
              {plan.warnings.length === 1
                ? "1 plugin will not be enabled."
                : `${plan.warnings.length} plugins will not be enabled.`}
            </b>
          </p>
          <ul aria-label="Plugins not enabled">
            {plan.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </Callout>
      )}
      <Callout icon={PackageOpen}>
        <p>
          <b>Plugins are not copied.</b> <code>.claude/settings.json</code>{" "}
          enables them; Claude Code offers to install them the first time you
          open this project and trust the folder. Look for the{" "}
          <code>enabledPlugins</code> lines in the settings preview.
        </p>
      </Callout>
    </>
  );
}

/** The same explanation for the Done step, after the files are written. */
export function PluginsDone() {
  return (
    <p>
      Plugins were not copied. <code>.claude/settings.json</code> enables them;
      Claude Code offers to install them the first time you open this project
      and trust the folder.
    </p>
  );
}
