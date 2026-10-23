import { Bot } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { CliStatus } from "../../bindings/CliStatus";
import { claudeCliStatus, settingsGet, settingsSet } from "../../lib/ipc";
import { Button, announce } from "../../ui";
import { Group } from "./Group";
import styles from "./Settings.module.css";
import { Switch } from "./Switch";

const MODEL_KEY = "bundles.model";
const AT_IMPORT_KEY = "bundles.atImport";

const missing: CliStatus = { found: false, path: null, version: null };

const models = [
  { value: "haiku", label: "Haiku (fast, the default)" },
  { value: "sonnet", label: "Sonnet" },
  { value: "opus", label: "Opus" },
] as const;

/** Settings → Claude: the CLI Harneo calls, the model it uses, and whether import asks it. */
export function ClaudeSettings() {
  const [cli, setCli] = useState<CliStatus | undefined>(undefined);
  const [model, setModel] = useState("haiku");
  const [atImport, setAtImport] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const modelId = useId();
  const toggleId = useId();

  /** Asks the backend again; the status row reads "Looking…" meanwhile. */
  function check() {
    setCli(undefined);
    claudeCliStatus().then(
      (status) => setCli(status ?? missing),
      () => setCli(missing),
    );
  }

  useEffect(() => {
    let live = true;
    claudeCliStatus().then(
      (status) => live && setCli(status ?? missing),
      () => live && setCli(missing),
    );
    settingsGet(MODEL_KEY).then(
      (value) => {
        if (live && typeof value === "string" && value !== "") setModel(value);
      },
      () => undefined,
    );
    settingsGet(AT_IMPORT_KEY).then(
      (value) => {
        if (live && typeof value === "boolean") setAtImport(value);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  async function save(key: string, value: string | boolean, undo: () => void) {
    setError(null);
    try {
      await settingsSet(key, value);
      announce("Setting saved");
    } catch (err) {
      undo();
      setError(String(err));
    }
  }

  const known = models.some((m) => m.value === model);
  const notFound = cli?.found === false;
  return (
    <Group
      id="claude-settings"
      title="Claude CLI"
      icon={<Bot size={15} aria-hidden="true" />}
    >
      <div className={styles.row}>
        <div className={styles.grow}>
          <b>
            {cli === undefined ? "Looking…" : cli.found ? "Found" : "Not found"}
          </b>
          <div className={styles.sec}>
            {notFound
              ? "Bundles and tags won't be suggested at import; add them by hand."
              : "Used for bundle detection and tag suggestions at import. Artifact text is sent to Claude through your own CLI; no API key."}
          </div>
          {notFound && (
            <div className={styles.hint}>
              Install Claude Code (https://code.claude.com), then Check again.
            </div>
          )}
        </div>
        {cli?.found && (
          <div className={styles.where}>
            <span className={styles.mono}>{cli.path}</span>
            {cli.version && <span className={styles.mono}>{cli.version}</span>}
          </div>
        )}
        {notFound && (
          <Button small onClick={check}>
            Check again
          </Button>
        )}
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={modelId}>
            <b>Model</b>
          </label>
          <div className={styles.sec}>
            Which model reads your artifacts. Haiku is fast and cheap; the
            larger ones find subtler links.
            {notFound && " Needs the Claude CLI."}
          </div>
        </div>
        <select
          id={modelId}
          className={styles.select}
          value={model}
          disabled={notFound}
          onChange={(e) => {
            const previous = model;
            setModel(e.target.value);
            void save(MODEL_KEY, e.target.value, () => setModel(previous));
          }}
        >
          {!known && <option value={model}>{model}</option>}
          {models.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </select>
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={toggleId}>
            <b>Detect bundles and tags at import</b>
          </label>
          <div className={styles.sec}>
            Off by default. When on, each import sends the artifacts' main files
            (up to 6,000 characters each) and the names and descriptions of your
            library to Claude through your own claude CLI, using your Claude
            plan. Artifacts flagged for secrets are sent by name and description
            only. When off, the Review step offers Detect now for that import.
            {notFound && " Needs the Claude CLI."}
          </div>
        </div>
        <Switch
          id={toggleId}
          checked={atImport}
          disabled={notFound}
          onChange={(next) => {
            setAtImport(next);
            void save(AT_IMPORT_KEY, next, () => setAtImport(!next));
          }}
        />
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </Group>
  );
}
