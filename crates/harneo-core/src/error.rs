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
        "{url} needs credentials or does not exist; Harneo never asks for a password, so use a \
         public URL or an SSH key loaded in your agent"
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
    },
    /// Claude answered with something that is not the JSON that was asked for, or too much of it.
    #[error("claude's answer could not be used: {message}")]
    ClaudeBadOutput {
        /// What is wrong with it.
        message: String,
    },
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
    /// An update cannot be made.
    #[error("cannot update: {message}")]
    Update {
        /// Why, ready to show.
        message: String,
    },
}

/// Result alias used across `harneo-core`.
pub type Result<T> = std::result::Result<T, Error>;
