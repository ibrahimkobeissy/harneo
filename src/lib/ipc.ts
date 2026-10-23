import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ImportReport } from "../bindings/ImportReport";
import type { Preview } from "../bindings/Preview";
import type { Source } from "../bindings/Source";
import type { ChangeScope } from "../bindings/ChangeScope";
import type { WatchStatus } from "../bindings/WatchStatus";
import type { AppInfo } from "../bindings/AppInfo";
import type { ArtifactAudit } from "../bindings/ArtifactAudit";
import type { ArtifactDetail } from "../bindings/ArtifactDetail";
import type { ArtifactKind } from "../bindings/ArtifactKind";
import type { DetectProgress } from "../bindings/DetectProgress";
import type { CliStatus } from "../bindings/CliStatus";
import type { Collection } from "../bindings/Collection";
import type { LinkOrigin } from "../bindings/LinkOrigin";
import type { Links } from "../bindings/Links";
import type { Detection } from "../bindings/Detection";
import type { Filter } from "../bindings/Filter";
import type { Page } from "../bindings/Page";
import type { Query } from "../bindings/Query";
import type { ReauditReport } from "../bindings/ReauditReport";
import type { IndexProgress } from "../bindings/IndexProgress";
import type { IndexReport } from "../bindings/IndexReport";
import type { Usage } from "../bindings/Usage";
import type { UsageSummary } from "../bindings/UsageSummary";
import type { CheckReport } from "../bindings/CheckReport";
import type { CheckScope } from "../bindings/CheckScope";
import type { FileDiff } from "../bindings/FileDiff";
import type { UpdateChoice } from "../bindings/UpdateChoice";
import type { UpdateReport } from "../bindings/UpdateReport";
import type { UpdateStatus } from "../bindings/UpdateStatus";
import type { Layout } from "../bindings/Layout";
import type { ProfileImportPreview } from "../bindings/ProfileImportPreview";
import type { ProfileImportReport } from "../bindings/ProfileImportReport";
import type { ProfileSpec } from "../bindings/ProfileSpec";
import type { ProfileSummary } from "../bindings/ProfileSummary";
import type { ProfileView } from "../bindings/ProfileView";
import type { ProfileUse } from "../bindings/ProfileUse";
import type { Resolved } from "../bindings/Resolved";
import type { ApplyReport } from "../bindings/ApplyReport";
import type { Backup } from "../bindings/Backup";
import type { InstalledFile } from "../bindings/InstalledFile";
import type { Comparison } from "../bindings/Comparison";
import type { Plan } from "../bindings/Plan";
import type { RefitAction } from "../bindings/RefitAction";
import type { Project } from "../bindings/Project";
import type { ProjectFile } from "../bindings/ProjectFile";
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

/** The stored audit of every library artifact (trust, findings, reach), for the Security page. */
export function libraryAudits(): Promise<ArtifactAudit[]> {
  return invoke<ArtifactAudit[]>("library_audits", {});
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

/**
 * Copies the ticked artifacts of a preview into the library, with the tags chosen for each and
 * the accepted bundle links as `[from, to, reason]`.
 */
export function libraryImport(
  preview: Preview,
  selection: string[],
  tags: Record<string, string[]>,
  links: [string, string, string][] = [],
): Promise<ImportReport> {
  // Without accepted links the argument is left out (the backend reads it as none).
  return invoke<ImportReport>(
    "library_import",
    links.length > 0
      ? { preview, selection, tags, links }
      : { preview, selection, tags },
  );
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

// --- Usage (session logs, names only) --------------------------------------------------

/** Reads new lines of Claude's session logs; resolves when done. */
export function usageIndex(): Promise<IndexReport> {
  return invoke<IndexReport>("usage_index", {});
}

/** Calls `handler` as an index run progresses; resolves with the function that stops listening. */
export function onUsageIndexProgress(
  handler: (progress: IndexProgress) => void,
): Promise<UnlistenFn> {
  return listen<IndexProgress>("usage-index-progress", (event) =>
    handler(event.payload),
  );
}

/** Sessions in the last 8 weeks, weekly graph, last use and projects of one artifact. */
export function usageOf(kind: ArtifactKind, name: string): Promise<Usage> {
  return invoke<Usage>("usage_of", { kind, name });
}

/** Sessions per week over every tracked use. */
export function usageSummary(): Promise<UsageSummary> {
  return invoke<UsageSummary>("usage_summary", {});
}

// --- Links, bundles and settings ---

/** What an artifact needs and what needs it (confirmed links). */
export function linksOf(id: string): Promise<Links> {
  return invoke<Links>("links_of", { id });
}

/** Confirms the link `from needs to`. */
export function linkAdd(
  from: string,
  to: string,
  origin: LinkOrigin,
  reason: string,
): Promise<void> {
  return invoke<void>("link_add", { from, to, origin, reason });
}

/** Deletes a link. */
export function linkRemove(from: string, to: string): Promise<void> {
  return invoke<void>("link_remove", { from, to });
}

/** Remembers a refused proposal so a re-run does not offer it again. */
export function linkReject(from: string, to: string): Promise<void> {
  return invoke<void>("link_reject", { from, to });
}

/** The artifact's bundle: everything it needs, transitively, without itself. */
export function bundleOf(id: string): Promise<string[]> {
  return invoke<string[]>("bundle_of", { id });
}

/** One app setting (keys are namespaced by module); `null` when unset. */
export function settingsGet(key: string): Promise<unknown> {
  return invoke<unknown>("settings_get", { key });
}

/** Stores one app setting. */
export function settingsSet(key: string, value: unknown): Promise<void> {
  return invoke<void>("settings_set", { key, value });
}

// --- Updates ---

/** Looks at the sources for artifacts that changed at their source; one entry per source. */
export function updatesCheck(
  scope: CheckScope = { type: "all" },
): Promise<CheckReport> {
  return invoke<CheckReport>("updates_check", { scope });
}

/** The update state of an artifact; `null` when it matches its source. */
export function updateStatus(id: string): Promise<UpdateStatus | null> {
  return invoke<UpdateStatus | null>("update_status", { id });
}
// --- Claude detection (bundles and tags) ---

/** Whether the `claude` CLI is on the PATH, and its version. */
export function claudeCliStatus(): Promise<CliStatus> {
  return invoke<CliStatus>("claude_cli_status");
}

/** Claude's proposed links and tags for the ticked candidates of a preview. Slow. */
export function bundlesDetectPreview(
  preview: Preview,
  selection: string[],
): Promise<Detection> {
  return invoke<Detection>("bundles_detect_preview", { preview, selection });
}

/** Claude's proposed links and tags for stored artifacts (`null` is the whole library). */
export function bundlesDetectLibrary(ids: string[] | null): Promise<Detection> {
  return invoke<Detection>("bundles_detect_library", { ids });
}

/** What updating an artifact would change, per file (`+`, `~`, `-` with hunks). */
export function updateDiff(id: string): Promise<FileDiff[]> {
  return invoke<FileDiff[]>("update_diff", { id });
}

/** Takes the upstream version (an edited copy is backed up first) or keeps the user's. */
export function updateApply(
  id: string,
  choice: UpdateChoice,
): Promise<UpdateReport> {
  return invoke<UpdateReport>("update_apply", { id, choice });
}

/** Updates every outdated artifact without local edits; edited ones come back skipped. */
export function updateAll(): Promise<UpdateReport[]> {
  return invoke<UpdateReport[]>("update_all");
}
/** Calls `handler` as a detection pass moves through its batches; resolves with the stop function. */
export function onBundlesDetectProgress(
  handler: (progress: DetectProgress) => void,
): Promise<UnlistenFn> {
  return listen<DetectProgress>("bundles-detect-progress", (event) =>
    handler(event.payload),
  );
}

// --- Profiles --------------------------------------------------------------------------
// Profile ids are `bigint` in the generated types but cross IPC as plain JSON numbers.

/** Every profile by name, with its type counts and the projects using it. */
export function profilesList(): Promise<ProfileSummary[]> {
  return invoke<ProfileSummary[]>("profiles_list");
}

/** A profile at `version` (the current one when omitted), with its history. */
export function profileGet(id: bigint, version?: number): Promise<ProfileView> {
  return invoke<ProfileView>("profile_get", {
    id: Number(id),
    version: version ?? null,
  });
}

/** Creates a profile at version 1; rejects on an invalid spec or a taken name. */
export function profileCreate(
  name: string,
  description: string,
  spec: ProfileSpec,
): Promise<bigint> {
  return invoke<number>("profile_create", { name, description, spec }).then(
    BigInt,
  );
}

/** Saves the spec as the next version; resolves with the version now current. */
export function profileSave(id: bigint, spec: ProfileSpec): Promise<number> {
  return invoke<number>("profile_save", { id: Number(id), spec });
}

/** Renames a profile (not a new version). */
export function profileRename(id: bigint, name: string): Promise<void> {
  return invoke<void>("profile_rename", { id: Number(id), name });
}

/** Changes a profile's description (not a new version). */
export function profileDescribe(
  id: bigint,
  description: string,
): Promise<void> {
  return invoke<void>("profile_describe", { id: Number(id), description });
}

/** Deletes a profile and all its versions. */
export function profileDelete(id: bigint): Promise<void> {
  return invoke<void>("profile_delete", { id: Number(id) });
}

/** Copies a profile under a new name; resolves with the copy's id. */
export function profileClone(id: bigint, name: string): Promise<bigint> {
  return invoke<number>("profile_clone", { id: Number(id), name }).then(BigInt);
}

/** A new profile holding a collection's artifacts. */
export function profileFromCollection(
  collectionId: bigint,
  name: string,
): Promise<bigint> {
  return invoke<number>("profile_from_collection", {
    collectionId: Number(collectionId),
    name,
  }).then(BigInt);
}

/** What a spec installs: bundles pulled in, Settings, MCP servers, missing ids. */
export function profileResolve(spec: ProfileSpec): Promise<Resolved> {
  return invoke<Resolved>("profile_resolve", { spec });
}

/** The file tree a spec installs, with conflicts. */
export function profileLayout(spec: ProfileSpec): Promise<Layout> {
  return invoke<Layout>("profile_layout", { spec });
}

/** The native save dialog for a profile file: the chosen path, or `null` on cancel. */
export async function pickProfileSavePath(
  defaultName: string,
): Promise<string | null> {
  return save({
    defaultPath: defaultName,
    filters: [{ name: "Harneo profile", extensions: ["json"] }],
  });
}

/** The native open dialog for a profile file: the chosen path, or `null` on cancel. */
export async function pickProfileFile(): Promise<string | null> {
  const chosen = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Harneo profile", extensions: ["json"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

/** Writes the profile (at `version`, else the current one) to `path`. */
export function profileExport(
  id: bigint,
  path: string,
  version?: number,
): Promise<void> {
  return invoke<void>("profile_export", {
    id: Number(id),
    version: version ?? null,
    path,
  });
}

/** Reads the profile file at `path` and says what importing it would do. */
export function profileImportPreview(
  path: string,
): Promise<ProfileImportPreview> {
  return invoke<ProfileImportPreview>("profile_import_preview", { path });
}

/** Imports the profile file at `path`; `name` overrides the file's own name. */
export function profileImportCommit(
  path: string,
  name?: string,
): Promise<ProfileImportReport> {
  return invoke<ProfileImportReport>("profile_import_commit", {
    path,
    name: name ?? null,
  });
}

/** Sets a skill's library default for manual-only invocation. */
export function artifactSetManualOnly(
  id: string,
  manualOnly: boolean,
): Promise<void> {
  return invoke<void>("artifact_set_manual_only", { id, manualOnly });
}
// --- Install engine: scaffold, apply, projects -----------------------------------------

/** What scaffolding a profile (the current version when omitted) into a folder would write. */
export function scaffoldPlan(
  target: string,
  profileId: bigint,
  version?: number,
): Promise<Plan> {
  return invoke<Plan>("scaffold_plan", {
    target,
    profileId: Number(profileId),
    version: version ?? null,
  });
}

/** A project's harness compared with a profile (the current version when omitted), item by item. */
export function refitCompare(
  target: string,
  profileId: bigint,
  version?: number,
): Promise<Comparison> {
  return invoke<Comparison>("refit_compare", {
    target,
    profileId: Number(profileId),
    version: version ?? null,
  });
}

/** The refit plan for the user's choices (item to action; unchosen items take their default). */
export function refitPlan(
  target: string,
  profileId: bigint,
  choices: Record<string, RefitAction>,
  version?: number,
): Promise<Plan> {
  return invoke<Plan>("refit_plan", {
    target,
    profileId: Number(profileId),
    version: version ?? null,
    choices,
  });
}

/** Writes a plan into its project, with backups first and a rollback on failure. */
export function installApply(plan: Plan): Promise<ApplyReport> {
  return invoke<ApplyReport>("install_apply", { plan });
}

/** Every project Harneo installed a profile into. */
export function projectsList(): Promise<Project[]> {
  return invoke<Project[]>("projects_list");
}

/** One project. */
export function projectGet(id: bigint): Promise<Project> {
  return invoke<Project>("project_get", { id: Number(id) });
}

/** The files Harneo wrote into a project. */
export function projectFiles(id: bigint): Promise<InstalledFile[]> {
  return invoke<InstalledFile[]>("project_files", { id: Number(id) });
}

/** The current content of one managed file of a project. */
export function projectFileContent(
  id: bigint,
  path: string,
): Promise<ProjectFile> {
  return invoke<ProjectFile>("project_file_content", {
    id: Number(id),
    path,
  });
}

/** The profiles whose current version installs the artifact (bundles included). */
export function artifactProfiles(id: string): Promise<ProfileUse[]> {
  return invoke<ProfileUse[]>("artifact_profiles", { id });
}
/** The plan to move a project to another profile or version: only the difference. */
export function switchPlan(
  projectId: bigint,
  profileId: bigint,
  version?: number,
): Promise<Plan> {
  return invoke<Plan>("switch_plan", {
    projectId: Number(projectId),
    profileId: Number(profileId),
    version: version ?? null,
  });
}

/** The plan to remove a project's profile: Harneo's files out, the originals back. */
export function removalPlan(projectId: bigint): Promise<Plan> {
  return invoke<Plan>("removal_plan", { projectId: Number(projectId) });
}

/** The backup sets of a project, newest first. */
export function projectBackups(id: bigint): Promise<Backup[]> {
  return invoke<Backup[]>("project_backups", { id: Number(id) });
}

/** Puts a backup set back into its project; the current versions are backed up first. */
export function backupRestore(backupId: bigint): Promise<ApplyReport> {
  return invoke<ApplyReport>("backup_restore", {
    backupId: Number(backupId),
  });
}

/** Deletes a backup set from disk and from the list. */
export function backupDelete(backupId: bigint): Promise<void> {
  return invoke<void>("backup_delete", { backupId: Number(backupId) });
}
// --- Live rescan ---

/** Calls `handler` when the library or a registered project changed on disk (debounced). */
export function onChanged(
  handler: (scope: ChangeScope) => void,
): Promise<UnlistenFn> {
  return listen<ChangeScope>("harneo://changed", (event) =>
    handler(event.payload),
  );
}

/** How many projects the live rescan watches, and whether it is paused. */
export function watchStatus(): Promise<WatchStatus> {
  return invoke<WatchStatus>("watch_status");
}

/** Pauses or resumes the live rescan (remembered). */
export function watchPause(paused: boolean): Promise<void> {
  return invoke<void>("watch_pause", { paused });
}
