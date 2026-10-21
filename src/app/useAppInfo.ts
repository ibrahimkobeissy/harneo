import { useEffect, useState } from "react";
import type { AppInfo } from "../bindings/AppInfo";
import { appInfo } from "../lib/ipc";

export type AppInfoState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; message: string };

export function useAppInfo(): AppInfoState {
  const [state, setState] = useState<AppInfoState>({ status: "loading" });

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
  }, []);

  return state;
}
