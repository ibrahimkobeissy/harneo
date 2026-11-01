import { Bug } from "lucide-react";
import { useEffect, useId, useState } from "react";
import { diagnosticsStatus, openExternal, settingsSet } from "../../lib/ipc";
import { announce } from "../../ui";
import { Group } from "./Group";
import styles from "./Settings.module.css";
import { Switch } from "./Switch";

const SWITCH_KEY = "diagnostics.sentryReports";
const BUILDING_URL =
  "https://github.com/ibrahimkobeissy/harneo/blob/main/BUILDING.md#error-reporting-optional";

/** Settings → Diagnostics: the opt-in switch for Sentry error reports. */
export function DiagnosticsSettings() {
  const [dsnMissing, setDsnMissing] = useState(false);
  const [on, setOn] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const id = useId();
  const helpId = useId();

  useEffect(() => {
    let live = true;
    diagnosticsStatus().then(
      (status) => {
        if (!live || !status) return;
        setDsnMissing(status.dsnMissing);
        setOn(status.switchOn);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  async function save(next: boolean) {
    setError(null);
    setOn(next);
    try {
      await settingsSet(SWITCH_KEY, next);
      announce("Setting saved");
    } catch (err) {
      setOn(!next);
      setError(String(err));
    }
  }

  return (
    <Group
      id="diagnostics"
      title="Diagnostics"
      icon={<Bug size={15} aria-hidden="true" />}
    >
      <div className={styles.row}>
        <div className={styles.grow}>
          <label htmlFor={id}>
            <b>Send error reports to Sentry</b>
          </label>
          <div id={helpId} className={styles.sec}>
            <p className={styles.para}>
              On by default once the HARNEO_SENTRY_DSN environment variable is
              set: setting a DSN is the opt-in. Turn it off here and it stays
              off. When on, crashes and errors in the app and the harneo command
              are sent to the Sentry project whose DSN is in that variable.
              Takes effect the next time Harneo starts. Sent:
            </p>
            <ul className={styles.list}>
              <li>the error message (up to 500 characters) and stack trace</li>
              <li>
                the Harneo version, the system type, and which part failed
              </li>
            </ul>
            <p className={styles.para}>
              Scrubbed before sending: your home folder becomes ~, request
              bodies, extra data and breadcrumb details are dropped, and any
              report containing a token or secret is not sent. File contents,
              audit evidence and Claude prompts are never attached. No
              performance data and no session recording.
            </p>
            {dsnMissing && (
              <p className={styles.para}>
                Needs a Sentry project: set the HARNEO_SENTRY_DSN environment
                variable to your project's DSN and restart Harneo. See{" "}
                <button
                  type="button"
                  className={styles.link}
                  onClick={() => {
                    openExternal(BUILDING_URL).catch((err: unknown) =>
                      setError(
                        `Could not open ${BUILDING_URL}: ${String(err)}`,
                      ),
                    );
                  }}
                >
                  BUILDING.md
                </button>
                .
              </p>
            )}
          </div>
        </div>
        <Switch
          id={id}
          checked={on}
          disabled={dsnMissing}
          describedBy={helpId}
          onChange={(next) => void save(next)}
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
