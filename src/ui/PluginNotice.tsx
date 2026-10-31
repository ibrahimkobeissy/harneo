import {
  CircleCheck,
  CircleX,
  PackageOpen,
  RotateCw,
  TriangleAlert,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Plan } from "../bindings/Plan";
import type { PluginInstall } from "../bindings/PluginInstall";
import type { PluginInstallStatus } from "../bindings/PluginInstallStatus";
import { onPluginsInstallProgress, projectPluginsInstall } from "../lib/ipc";
import { Button } from "./Button";
import { Callout } from "./Callout";
import { decodeText } from "./CodeView";
import styles from "./PluginsInstall.module.css";

/** HARNESS.md carries this sentence whenever the profile holds a plugin. */
const MARK = "Plugins are not copied: they install from their own marketplace.";

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
          <b>
            Plugins are not copied: they install from their own marketplace.
          </b>{" "}
          <code>.claude/settings.json</code> enables them (see the{" "}
          <code>enabledPlugins</code> lines in the settings preview), and once
          the files are written Harneo installs them for this project through
          your <code>claude</code> CLI.
        </p>
      </Callout>
    </>
  );
}

/** What a result says next to the plugin's name. */
const statusText: Record<PluginInstallStatus, string> = {
  installed: "installed for this project",
  alreadyInstalled: "already installed",
  failed: "not installed",
};

/**
 * The Done step's plugins: installs every plugin the project's settings enable, from its own
 * marketplace, as soon as the step shows, with each result as it lands and a Retry when one
 * failed. Nothing is copied; Claude Code does the install.
 */
export function PluginsInstall({ projectId }: { projectId: bigint }) {
  const [results, setResults] = useState<PluginInstall[]>([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // One run per mount, even when React runs the effect twice in development.
  const started = useRef(false);

  const run = useCallback(async () => {
    setRunning(true);
    setError(null);
    setResults([]);
    const stop = await onPluginsInstallProgress((r) =>
      setResults((prev) => [...prev.filter((p) => p.id !== r.id), r]),
    );
    try {
      setResults(await projectPluginsInstall(projectId));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      stop();
      setRunning(false);
    }
  }, [projectId]);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void run();
  }, [run]);

  const failed = results.filter((r) => r.status === "failed").length;
  return (
    <section className={styles.panel} aria-label="Plugins">
      <div className={styles.head} role="status">
        {running ? (
          <span className={styles.spinner} aria-hidden="true" />
        ) : (
          <PackageOpen size={16} aria-hidden="true" />
        )}
        {running
          ? "Installing plugins from their marketplaces…"
          : failed > 0
            ? `${failed} ${failed === 1 ? "plugin" : "plugins"} not installed`
            : "Plugins ready"}
      </div>
      {results.length > 0 && (
        <ul className={styles.list}>
          {results.map((r) => (
            <li key={r.id} className={styles.row}>
              {r.status === "failed" ? (
                <CircleX size={16} className={styles.bad} aria-hidden="true" />
              ) : (
                <CircleCheck
                  size={16}
                  className={styles.ok}
                  aria-hidden="true"
                />
              )}
              <span>
                <span className={styles.name}>{r.id}</span>{" "}
                <span className={styles.state}>{statusText[r.status]}</span>
                {r.message && <span className={styles.why}>{r.message}</span>}
              </span>
            </li>
          ))}
        </ul>
      )}
      {error && (
        <p className={styles.why} role="alert">
          {error}
        </p>
      )}
      {!running && (failed > 0 || error) && (
        <div>
          <Button small onClick={() => void run()}>
            <RotateCw size={14} aria-hidden="true" />
            Retry
          </Button>
        </div>
      )}
      <p className={styles.note}>
        Installed through your <code>claude</code> CLI from each plugin&apos;s
        own marketplace; nothing is copied. Anyone else who opens the project is
        offered them by Claude Code.
      </p>
    </section>
  );
}
