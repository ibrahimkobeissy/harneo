import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "../bindings/AppInfo";
import type { ArtifactDetail } from "../bindings/ArtifactDetail";
import type { ArtifactKind } from "../bindings/ArtifactKind";
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

// --- Artifact detail panel ---

/** One library artifact in full. */
export function libraryDetail(id: string): Promise<ArtifactDetail> {
  return invoke<ArtifactDetail>("library_detail", { id });
}

/** Deletes an artifact from the library. */
export function libraryRemove(id: string): Promise<void> {
  return invoke<void>("library_remove", { id });
}

/** The id of the library artifact of this kind and name, or null when it is not imported. */
export function libraryFind(
  kind: ArtifactKind,
  name: string,
): Promise<string | null> {
  return invoke<string | null>("library_find", { kind, name });
}
