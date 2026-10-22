import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportReport } from "../bindings/ImportReport";
import type { Preview } from "../bindings/Preview";
import type { Source } from "../bindings/Source";
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

// --- Import flow -----------------------------------------------------------------------

/** `~/.claude` as an absolute path. */
export function homeClaudeDir(): Promise<string> {
  return invoke<string>("home_claude_dir");
}

/**
 * Scans a source and lists what it holds. A git source is cloned first, so this can take a
 * while; the clone waits in the library for `libraryImport`.
 */
export function libraryPreview(source: Source): Promise<Preview> {
  return invoke<Preview>("library_preview", { source });
}

/** Copies the ticked artifacts of a preview into the library, with the tags chosen for each. */
export function libraryImport(
  preview: Preview,
  selection: string[],
  tags: Record<string, string[]>,
): Promise<ImportReport> {
  return invoke<ImportReport>("library_import", { preview, selection, tags });
}

/** The native folder picker: the chosen path, or `null` when the user cancels. */
export async function pickFolder(): Promise<string | null> {
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}
