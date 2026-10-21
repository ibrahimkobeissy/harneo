import { useCallback, useEffect, useState } from "react";
import type { AppInfo } from "../bindings/AppInfo";
import { appInfo } from "../lib/ipc";

export type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; message: string };

/** The app's version and data folder, with a retry for when the backend call fails. */
export function useAppInfo(): AppInfoState & { retry: () => void } {
  const [state, setState] = useState<AppInfoState>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    appInfo().then(
      (info) => {
        if (active) setState({ status: "ready", info });
      },
      (err: unknown) => {
        if (active) setState({ status: "error", message: String(err) });
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);

  const retry = useCallback(() => {
    setState({ status: "loading" });
    setAttempt((n) => n + 1);
  }, []);

  return { ...state, retry };
}
