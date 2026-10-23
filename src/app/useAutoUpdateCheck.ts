import { useEffect } from "react";
import { autoCheckIfDue } from "../lib/updates";

/**
 * Checks the library's sources once when the app starts, if the setting allows it and the last
 * check is more than a day old. Runs in the background; a failure is left for the next manual
 * check to report.
 */
export function useAutoUpdateCheck() {
  useEffect(() => {
    autoCheckIfDue().catch(() => {});
  }, []);
}
