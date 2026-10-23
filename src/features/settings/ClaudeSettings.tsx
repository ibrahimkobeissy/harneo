import { Bot } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { CliStatus } from "../../bindings/CliStatus";
import { claudeCliStatus, settingsGet, settingsSet } from "../../lib/ipc";
import { announce } from "../../ui";
import styles from "./Settings.module.css";

const MODEL_KEY = "bundles.model";
const AT_IMPORT_KEY = "bundles.atImport";

const models = [
  { value: "haiku", label: "Haiku (fast, the default)" },
  { value: "sonnet", label: "Sonnet" },
  { value: "opus", label: "Opus" },
] as const;

/** Settings → Claude: the CLI Harneo calls, the model it uses, and whether import asks it. */
export function ClaudeSettings() {
  const [cli, setCli] = useState<CliStatus | undefined>(undefined);
  const [model, setModel] = useState("haiku");
  const [atImport, setAtImport] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const modelId = useId();
  const toggleId = useId();

  useEffect(() => {
    let live = true;
    claudeCliStatus().then(
      (status) =>
        live && setCli(status ?? { found: false, path: null, version: null }),
      () => live && setCli({ found: false, path: null, version: null }),
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
  return (
    <section className={styles.group} aria-labelledby="claude-settings">
      <header className={styles.ghead}>
        <Bot size={15} aria-hidden="true" />
        <h2 id="claude-settings">Claude CLI</h2>
      </header>
      <div className={styles.row}>
        <div className={styles.grow}>
          <b>
            {cli === undefined ? "Looking…" : cli.found ? "Found" : "Not found"}
          </b>
          <div className={styles.sec}>
            {cli?.found === false
              ? "Bundles and tags won't be suggested at import; add them by hand."
              : "Used for bundle detection and tag suggestions at import. Artifact text is sent to Claude through your own CLI; no API key."}
          </div>
        </div>
        {cli?.found && (
          <div className={styles.where}>
            <span className={styles.mono}>{cli.path}</span>
            {cli.version && <span className={styles.mono}>{cli.version}</span>}
          </div>
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
          </div>
        </div>
        <select
          id={modelId}
          className={styles.select}
          value={model}
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
            When off, the Review step offers a Detect now button instead.
          </div>
        </div>
        <button
          id={toggleId}
          type="button"
          role="switch"
          aria-checked={atImport}
          className={styles.switch}
          onClick={() => {
            const next = !atImport;
            setAtImport(next);
            void save(AT_IMPORT_KEY, next, () => setAtImport(!next));
          }}
        >
          <i />
        </button>
      </div>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
    </section>
  );
}
