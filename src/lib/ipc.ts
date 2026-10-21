import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "../bindings/AppInfo";

/** The only module that calls `invoke`: one typed wrapper per backend command. */
export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
