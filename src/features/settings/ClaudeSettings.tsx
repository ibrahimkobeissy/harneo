import { Bot } from "lucide-react";
import { useEffect, useId, useState } from "react";
import type { CliStatus } from "../../bindings/CliStatus";
import {
  auditConfirmSettingGet,
  auditConfirmSettingSet,
  claudeCliStatus,
  importLayoutSettingGet,
  importLayoutSettingSet,
  openExternal,
  settingsGet,
  settingsSet,
} from "../../lib/ipc";
import { Button, announce } from "../../ui";
import { CopyPath } from "./CopyPath";
import { Group } from "./Group";
import styles from "./Settings.module.css";
import { Switch } from "./Switch";

const MODEL_KEY = "bundles.model";
const AT_IMPORT_KEY = "bundles.atImport";

const INSTALL_URL = "https://code.claude.com";
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
  const [confirm, setConfirm] = useState(true);
  const [layout, setLayout] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const layoutId = useId();
  const layoutHelpId = useId();
  const confirmId = useId();
  const confirmHelpId = useId();
  const modelId = useId();
  const toggleId = useId();
  const modelHelpId = useId();
  const toggleHelpId = useId();

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
    auditConfirmSettingGet().then(
      (value) => {
        if (live && typeof value === "boolean") setConfirm(value);
      },
      () => undefined,
    );
    importLayoutSettingGet().then(
      (value) => {
        if (live && typeof value === "boolean") setLayout(value);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  async function saveLayout(next: boolean) {
    setError(null);
    setLayout(next);
    try {
      await importLayoutSettingSet(next);
      announce("Setting saved");
    } catch (err) {
      setLayout(!next);
      setError(String(err));
    }
  }

  async function saveConfirm(next: boolean) {
    setError(null);
    setConfirm(next);
    try {
      await auditConfirmSettingSet(next);
      announce("Setting saved");
    } catch (err) {
      setConfirm(!next);
      setError(String(err));
    }
  }

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
      title="Claude"
      icon={<Bot size={15} aria-hidden="true" />}
    >
      <div className={styles.row}>
        <div className={styles.grow}>
          <b>Claude CLI</b>
          <div className={styles.sec}>
            <span className={styles.status}>
              {cli === undefined
                ? "Looking…"
                : cli.found
                  ? "Found"
                  : "Not found"}
            </span>
            {notFound
              ? ". Bundles and tags won't be suggested at import; add them by hand."
              : ". Used for bundle detection and tag suggestions at import, through your own CLI; no API key."}
          </div>
          {notFound && (
            <div className={styles.hint}>
              <span>
                Install Claude Code from{" "}
                <button
                  type="button"
                  className={styles.link}
                  onClick={() => {
                    openExternal("claudeInstall").catch((err: unknown) =>
                      setError(`Could not open ${INSTALL_URL}: ${String(err)}`),
                    );
                  }}
                >
                  code.claude.com
                </button>
                , then Check again.
              </span>
            </div>
          )}
        </div>
        {cli?.found && (
          <div className={styles.where}>
            <span className={styles.mono}>{cli.path}</span>
            {cli.version && <span className={styles.mono}>{cli.version}</span>}
          </div>
        )}
        {cli?.found && cli.path && (
          <CopyPath path={cli.path} label="Copy the Claude CLI path" />
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
          <div id={modelHelpId} className={styles.sec}>
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
          aria-describedby={modelHelpId}
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
          <label htmlFor={confirmId}>
            <b>Confirm security findings with Claude</b>
          </label>
          <div id={confirmHelpId} className={styles.sec}>
            <p className={styles.para}>
              On by default. After an import or update, and on Confirm now in
              Security, each finding is sent to Claude through your own claude
              CLI, using your Claude plan, with the model chosen above:
            </p>
            <ul className={styles.list}>
              <li>
                each finding&apos;s rule, file and line, and a few lines around
                it
              </li>
              <li>the artifact&apos;s name and description</li>
            </ul>
            <p className={styles.para}>
              Claude can only keep or dismiss a finding. Turn it off and nothing
              is sent; Security then warns that the audit is phase 1 only.
              {notFound && " Needs the Claude CLI."}
            </p>
          </div>
        </div>
        <Switch
          id={confirmId}
          checked={confirm}
          describedBy={confirmHelpId}
          onChange={(next) => void saveConfirm(next)}
        />
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={toggleId}>
            <b>Detect bundles and tags at import</b>
          </label>
          <div id={toggleHelpId} className={styles.sec}>
            <p className={styles.para}>
              Off by default. When on, each import sends these to Claude through
              your own claude CLI, using your Claude plan:
            </p>
            <ul className={styles.list}>
              <li>
                each artifact&apos;s main files (up to 6,000 characters each)
              </li>
              <li>the names and descriptions of your library</li>
            </ul>
            <p className={styles.para}>
              Artifacts flagged for secrets are sent by name and description
              only. When off, the Review step offers Detect now for that import.
              {notFound && " Needs the Claude CLI."}
            </p>
          </div>
        </div>
        <Switch
          id={toggleId}
          checked={atImport}
          disabled={notFound}
          describedBy={toggleHelpId}
          onChange={(next) => {
            setAtImport(next);
            void save(AT_IMPORT_KEY, next, () => setAtImport(!next));
          }}
        />
      </div>
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={layoutId}>
            <b>Ask Claude about repo layouts at import</b>
          </label>
          <div id={layoutHelpId} className={styles.sec}>
            <p className={styles.para}>
              On by default. An import that leaves files Harneo couldn&apos;t
              place sends these to Claude through your own claude CLI, using
              your Claude plan:
            </p>
            <ul className={styles.list}>
              <li>the repo&apos;s file paths (up to 2,000)</li>
              <li>the text of its manifests</li>
              <li>
                the first 40 lines of each Markdown file Harneo couldn&apos;t
                place (up to 60 files)
              </li>
            </ul>
            <p className={styles.para}>
              Files that look like secrets are listed by name only. When off,
              the Choose step offers Ask Claude what&apos;s in this repo for
              that import.
              {notFound && " Needs the Claude CLI."}
            </p>
          </div>
        </div>
        <Switch
          id={layoutId}
          checked={layout}
          disabled={notFound}
          describedBy={layoutHelpId}
          onChange={(next) => void saveLayout(next)}
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
