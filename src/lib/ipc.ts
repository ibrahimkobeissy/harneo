import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "../bindings/AppInfo";
import type { Page } from "../bindings/Page";
import type { Query } from "../bindings/Query";

/** The only module that calls `invoke`: one typed wrapper per backend command. */
export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

/** One page of the library: filtered, sorted, with its total and facet counts. */
export function libraryList(query: Query): Promise<Page> {
  return invoke<Page>("library_list", { query });
}
