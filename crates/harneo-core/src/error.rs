//! The error type shared by every core module, so the app and the CLI report
//! failures the same way.

/// Everything that can go wrong in `harneo-core`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The OS gave no per-user data location, so there is nowhere to keep the library.
    #[error("cannot determine the user data directory on this system")]
    NoDataDir,
    /// The OS gave no home folder, so `~/.claude` cannot be found.
    #[error("cannot determine the home directory on this system")]
    NoHomeDir,
    /// The folder to scan is missing or is not a folder.
    #[error("cannot scan {path}: not a folder")]
    NotAFolder {
        /// The path as the caller gave it.
        path: String,
    },
    /// A read failed on a folder that was being scanned.
    #[error("cannot read {path}: {source}")]
    Io {
        /// What was being read, relative to the scan root.
        path: String,
        /// The underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// The library index could not be opened, migrated or queried.
    #[error("library database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// The library was written by a newer Harneo than this one.
    #[error(
        "the library uses schema version {found}, but this Harneo only understands up to \
         {supported}; update Harneo to open it"
    )]
    SchemaTooNew {
        /// The version stored in the database.
        found: u32,
        /// The newest version this build can read.
        supported: u32,
    },
    /// An import asked for an artifact that is not in the preview it was given, or whose id does
    /// not match its source and path.
    #[error("artifact {id} is not part of this preview")]
    NotInPreview {
        /// The id that was selected.
        id: String,
    },
    /// A plugin and its parts cannot both be in the library: the import would have both.
    #[error("{plugin}: {detail}")]
    PluginExclusive {
        /// The plugin's name.
        plugin: String,
        /// What conflicts and what to remove first.
        detail: String,
    },
    /// The artifact asked to unpack is not a plugin held whole.
    #[error("{name} is not a plugin in the library")]
    NotAPlugin {
        /// The artifact's name.
        name: String,
    },
    /// A part of an unpacked plugin refers to files outside itself, so it only works with the
    /// whole plugin.
    #[error("{name} only works with its whole plugin: it reaches files outside itself")]
    PartNeedsPlugin {
        /// The part's name.
        name: String,
    },
    /// A path from a scanned source was absolute, climbed out with `..`, or went through a
    /// symlink, so it was not copied.
    #[error("unsafe path in the source: {path}")]
    UnsafePath {
        /// The path as the scan reported it.
        path: String,
    },
    /// A Settings file in the source is not valid JSON, so its literal secrets cannot be found
    /// and replaced; the artifact is not copied.
    #[error("cannot copy {path}: not valid JSON, so secrets in it cannot be replaced")]
    UnreadableSettings {
        /// The file, relative to the source root.
        path: String,
    },
    /// A link or setting the library refuses.
    #[error("{message}")]
    InvalidLink {
        /// What is wrong.
        message: String,
    },
    /// The artifact is not in the library.
    #[error("artifact {id} is not in the library")]
    NotInLibrary {
        /// The id that was asked for.
        id: String,
    },
    /// A stored artifact record could not be read back.
    #[error("library record is damaged: {0}")]
    BadRecord(#[from] serde_json::Error),
    /// The text is not a git URL Harneo understands.
    #[error("not a git URL: {input}")]
    InvalidGitUrl {
        /// The input, with credentials removed.
        input: String,
    },
    /// The `git` program is missing.
    #[error("git is not installed, or is not on the PATH")]
    GitNotInstalled,
    /// The repository does not exist, is not a git repository, or cannot be reached.
    #[error("cannot reach the git repository {url}: it does not exist, is not reachable, or is not a git repository")]
    GitRepoUnreachable {
        /// The URL, with credentials removed.
        url: String,
    },
    /// The repository needs a login, or does not exist (hosts answer the same for both).
    #[error(
        "{url} needs credentials or does not exist; Harneo never asks for a password: for a \
         private repo, use its SSH URL (git@host:owner/repo.git) with a key loaded in your SSH \
         agent, or a git credential helper for HTTPS"
    )]
    GitAuthRequired {
        /// The URL, with credentials removed.
        url: String,
    },
    /// The branch (or tag) is not in the repository.
    #[error("the repository has no branch or tag named {branch}")]
    GitBranchNotFound {
        /// The branch as asked for.
        branch: String,
    },
    /// The subfolder is not in the repository at that branch.
    #[error("the repository has no folder {subfolder}")]
    GitSubfolderMissing {
        /// The subfolder as asked for.
        subfolder: String,
    },
    /// Cloning took longer than the limit and was stopped.
    #[error("git did not finish within {seconds} s and was stopped")]
    GitTimeout {
        /// The limit, in whole seconds.
        seconds: u64,
    },
    /// Git failed for a reason Harneo does not single out.
    #[error("git failed: {message}")]
    GitFailed {
        /// Git's last error lines, with credentials removed.
        message: String,
    },
    /// The source was scanned and holds no artifact.
    #[error("no artifacts found in {source_name}")]
    NothingFound {
        /// The source, as shown to the user.
        source_name: String,
    },
    /// A git preview is gone (replaced by a newer one, or already imported): preview again.
    #[error("this git preview is no longer available; preview the source again")]
    PreviewExpired,
    /// A tag name or color is not acceptable.
    #[error("invalid tag: {reason}")]
    InvalidTag {
        /// What is wrong, ready to show.
        reason: String,
    },
    /// Another tag already has this name (names are unique ignoring case).
    #[error("a tag named {name} already exists")]
    TagExists {
        /// The name that clashed.
        name: String,
    },
    /// No tag has this id.
    #[error("tag {id} does not exist")]
    TagNotFound {
        /// The id that was asked for.
        id: i64,
    },
    /// A collection name, rule or order is not acceptable.
    #[error("invalid collection: {reason}")]
    InvalidCollection {
        /// What is wrong, ready to show.
        reason: String,
    },
    /// Another collection already has this name (names are unique ignoring case).
    #[error("a collection named {name} already exists")]
    CollectionExists {
        /// The name that clashed.
        name: String,
    },
    /// No collection has this id.
    #[error("collection {id} does not exist")]
    CollectionNotFound {
        /// The id that was asked for.
        id: i64,
    },
    /// The `claude` program is missing.
    #[error("the claude CLI is not installed, or is not on the PATH")]
    ClaudeMissing,
    /// Detection was asked for without the user's consent: the `bundles.atImport` setting is off
    /// and the call was not started by the user.
    #[error("detection is off: turn it on in Settings, or start it yourself")]
    DetectionNotAllowed,
    /// A claude call ran past the limit and was stopped.
    #[error("claude did not finish within {seconds} s and was stopped")]
    ClaudeTimeout {
        /// The limit, in whole seconds.
        seconds: u64,
    },
    /// Claude exited with an error, or could not be started.
    #[error("claude failed: {message}")]
    ClaudeFailed {
        /// Claude's last error text, cut short; never the prompt.
        message: String,
        /// What sort of failure, when it could be told.
        kind: crate::bundles::FailureKind,
    },
    /// Claude answered with something that is not the JSON that was asked for, or too much of it.
    #[error("claude's answer could not be used: {message}")]
    ClaudeBadOutput {
        /// What is wrong with it.
        message: String,
    },
    /// The user cancelled a draw; nothing was saved.
    #[error("cancelled")]
    Cancelled,
    /// Claude's map did not pass the checks twice; nothing was saved.
    #[error("Answer didn't check out: {}", reasons.join("; "))]
    BadAnswer {
        /// Every reason the last answer failed, in plain words.
        reasons: Vec<String>,
    },
    /// The profile has no drawn map yet, so there is nothing to edit.
    #[error("the profile has no map yet: draw it first")]
    NoMap,
    /// A plugin taken whole cannot be fitted yet.
    #[error("Harnfit does not take a plugin whole yet; fit its skills, commands or agents one at a time")]
    HarnfitPluginUnsupported,
    /// A fit that needs the artifact in the library was asked for a candidate that is not there.
    #[error("import it into the library first")]
    ImportFirst,
    /// The candidate is already in the profile, so there is nothing to fit.
    #[error("{name} is already in the profile")]
    HarnfitAlreadyInProfile {
        /// The candidate's name.
        name: String,
    },
    /// No saved fit for this candidate.
    #[error("there is no saved fit for this candidate")]
    FitNotFound,
    /// The profile has no draw with this number.
    #[error("draw {no} does not exist")]
    DrawNotFound {
        /// The number that was asked for.
        no: u32,
    },
    /// A map edit was refused; nothing was changed.
    #[error("map edit refused: {reason}")]
    InvalidMapEdit {
        /// What is wrong, ready to show.
        reason: String,
    },
    /// The profile has no artifacts to put on a map.
    #[error("the profile has no artifacts to draw")]
    EmptyProfile,
    /// A profile name or spec is not acceptable.
    #[error("invalid profile: {reason}")]
    InvalidProfile {
        /// What is wrong, ready to show.
        reason: String,
    },
    /// Another profile already has this name (names are unique ignoring case).
    #[error("a profile named {name} already exists")]
    ProfileExists {
        /// The name that clashed.
        name: String,
    },
    /// No profile has this id, or it has no such version.
    #[error("profile {id} does not exist")]
    ProfileNotFound {
        /// The id that was asked for.
        id: i64,
    },
    /// The folder already has a harness (`.claude/`, `CLAUDE.md` or `.mcp.json`).
    #[error("{path} already has a harness ({found}); refit it instead")]
    HasHarness {
        /// The project folder.
        path: String,
        /// What was found there.
        found: String,
    },
    /// Another Harneo (the app or the CLI) is installing into a project.
    #[error("Harneo is busy: another install is running; try again in a moment")]
    Busy,
    /// A plan or a path in it is refused before anything is written.
    #[error("cannot install: {message}")]
    Install {
        /// Why, ready to show.
        message: String,
    },
    /// An install failed part-way and was undone (or could not be fully undone).
    #[error("{message}")]
    InstallFailed {
        /// What failed and whether the project was restored.
        message: String,
    },
    /// A reset of Harneo's data was refused or stopped part-way.
    #[error("{message}")]
    Reset {
        /// What happened, and what was and was not deleted.
        message: String,
    },
    /// A drift action is refused (a generated or deleted file cannot be promoted...).
    #[error("{message}")]
    Drift {
        /// Why, ready to show.
        message: String,
    },
    /// No project has this id.
    #[error("project {id} does not exist")]
    ProjectNotFound {
        /// The id that was asked for.
        id: i64,
    },
    /// No backup set has this id.
    #[error("backup {id} does not exist")]
    BackupNotFound {
        /// The id that was asked for.
        id: i64,
    },
    /// The profile is installed in projects, so it cannot be deleted.
    #[error("profile {name} is used by {projects}; remove it from them first")]
    ProfileInUse {
        /// The profile.
        name: String,
        /// The project names, comma separated.
        projects: String,
    },
    /// An update cannot be made.
    #[error("cannot update: {message}")]
    Update {
        /// Why, ready to show.
        message: String,
    },
}

/// Result alias used across `harneo-core`.
pub type Result<T> = std::result::Result<T, Error>;
