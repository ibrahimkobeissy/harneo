import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { ImportReport } from "../bindings/ImportReport";
import type { Preview } from "../bindings/Preview";
import type { Source } from "../bindings/Source";
import type { AppInfo } from "../bindings/AppInfo";
import type { ArtifactDetail } from "../bindings/ArtifactDetail";
import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { Collection } from "../bindings/Collection";
import type { Filter } from "../bindings/Filter";
import type { Page } from "../bindings/Page";
import type { Query } from "../bindings/Query";
import type { ReauditReport } from "../bindings/ReauditReport";
import type { Tag } from "../bindings/Tag";
import type { TagColor } from "../bindings/TagColor";
import type { TagWithCount } from "../bindings/TagWithCount";

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

/** Re-runs the security audit on every stored copy; resolves with how many were audited and changed. */
export function libraryReaudit(): Promise<ReauditReport> {
  return invoke<ReauditReport>("library_reaudit", {});
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
export function libraryPreview(
  source: Source,
  includeManaged = false,
): Promise<Preview> {
  return invoke<Preview>("library_preview", { source, includeManaged });
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

// --- Tags ------------------------------------------------------------------------------
// Tag ids are `bigint` in the generated types but cross IPC as plain JSON numbers.

/** Every tag with the number of artifacts carrying it. */
export function tagsList(): Promise<TagWithCount[]> {
  return invoke<TagWithCount[]>("tags_list");
}

/** Creates a tag; rejects when the name is taken (ignoring case) or invalid. */
export function tagCreate(name: string, color: TagColor): Promise<Tag> {
  return invoke<Tag>("tag_create", { name, color });
}

/** Renames a tag. */
export function tagRename(id: bigint, name: string): Promise<Tag> {
  return invoke<Tag>("tag_rename", { id: Number(id), name });
}

/** Changes a tag's color. */
export function tagRecolor(id: bigint, color: TagColor): Promise<Tag> {
  return invoke<Tag>("tag_recolor", { id: Number(id), color });
}

/** Deletes a tag and takes it off every artifact. */
export function tagDelete(id: bigint): Promise<void> {
  return invoke<void>("tag_delete", { id: Number(id) });
}

/** The tags on one artifact. */
export function artifactTags(id: string): Promise<Tag[]> {
  return invoke<Tag[]>("artifact_tags", { id });
}

/** Puts a tag on an artifact. */
export function artifactTagAdd(id: string, tagId: bigint): Promise<void> {
  return invoke<void>("artifact_tag_add", { id, tagId: Number(tagId) });
}

/** Takes a tag off an artifact. */
export function artifactTagRemove(id: string, tagId: bigint): Promise<void> {
  return invoke<void>("artifact_tag_remove", { id, tagId: Number(tagId) });
}

// --- Collections -----------------------------------------------------------------------
// Collection ids are `bigint` in the generated types but cross IPC as plain JSON numbers.

/** Every collection by name, with its member count. */
export function collectionsList(): Promise<Collection[]> {
  return invoke<Collection[]>("collections_list");
}

/** Creates an empty manual collection; rejects on an invalid or taken name. */
export function collectionCreateManual(name: string): Promise<Collection> {
  return invoke<Collection>("collection_create_manual", { name });
}

/** Creates a smart collection whose members are whatever matches `rule`. */
export function collectionCreateSmart(
  name: string,
  rule: Filter,
): Promise<Collection> {
  return invoke<Collection>("collection_create_smart", { name, rule });
}

/** Renames a collection. */
export function collectionRename(
  id: bigint,
  name: string,
): Promise<Collection> {
  return invoke<Collection>("collection_rename", { id: Number(id), name });
}

/** Replaces a smart collection's rule. */
export function collectionUpdateRule(
  id: bigint,
  rule: Filter,
): Promise<Collection> {
  return invoke<Collection>("collection_update_rule", {
    id: Number(id),
    rule,
  });
}

/** Deletes a collection; its artifacts stay in the library. */
export function collectionDelete(id: bigint): Promise<void> {
  return invoke<void>("collection_delete", { id: Number(id) });
}

/** Appends an artifact to a manual collection. */
export function collectionAdd(
  collectionId: bigint,
  artifactId: string,
): Promise<void> {
  return invoke<void>("collection_add", {
    collectionId: Number(collectionId),
    artifactId,
  });
}

/** Takes an artifact out of a manual collection. */
export function collectionRemove(
  collectionId: bigint,
  artifactId: string,
): Promise<void> {
  return invoke<void>("collection_remove", {
    collectionId: Number(collectionId),
    artifactId,
  });
}

/** One page of a collection's members; `position` sort is a manual collection's own order. */
export function collectionPage(
  collectionId: bigint,
  query: Query,
): Promise<Page> {
  return invoke<Page>("collection_page", {
    collectionId: Number(collectionId),
    query,
  });
}

/** The ids of the manual collections an artifact is in. */
export function artifactCollections(id: string): Promise<number[]> {
  return invoke<number[]>("artifact_collections", { id });
}
