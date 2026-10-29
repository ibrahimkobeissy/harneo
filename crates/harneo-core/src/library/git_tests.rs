//! Git sources against local repositories built in temp folders, reached by `file://` URL.
//! Nothing here touches the network.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::git::{clone, ls_remote};
use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn git(dir: &Path, args: &[&str]) -> io::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn agent(name: &str) -> String {
    format!("---\nname: {name}\ndescription: {name} agent\n---\nGo.\n")
}

struct Fixture {
    _repo: tempfile::TempDir,
    url: String,
    main_sha: String,
    dev_sha: String,
    data: tempfile::TempDir,
    library: Library,
}

/// A repo with `agents/root.md` and (a nested root) `pack/agents/inner.md` on `main`, and a `dev` branch that
/// adds `agents/dev.md`.
fn fixture() -> std::result::Result<Fixture, Box<dyn std::error::Error>> {
    let repo = tempfile::tempdir()?;
    let dir = repo.path();
    git(dir, &["init", "-q", "-b", "main"])?;
    write(dir, "agents/root.md", &agent("root"))?;
    write(dir, "pack/agents/inner.md", &agent("inner"))?;
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", "one"])?;
    let main_sha = git(dir, &["rev-parse", "HEAD"])?;
    git(dir, &["checkout", "-q", "-b", "dev"])?;
    write(dir, "agents/dev.md", &agent("dev"))?;
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-q", "-m", "two"])?;
    let dev_sha = git(dir, &["rev-parse", "HEAD"])?;
    git(dir, &["checkout", "-q", "main"])?;
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    Ok(Fixture {
        url: format!("file://{}", dir.display()),
        _repo: repo,
        main_sha,
        dev_sha,
        data,
        library,
    })
}

fn source(f: &Fixture, branch: Option<&str>, sub: Option<&str>) -> Result<Source> {
    Source::parse_git(&f.url, branch, sub)
}

/// Entries of `library/` that are clone folders.
fn clones_left(data: &Path) -> io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(data.join("library"))? {
        if entry?.file_name().to_string_lossy().starts_with(".clone-") {
            count += 1;
        }
    }
    Ok(count)
}

fn names(preview: &Preview) -> Vec<String> {
    let mut names: Vec<String> = preview
        .candidates
        .iter()
        .map(|c| c.artifact.name.clone())
        .collect();
    names.sort();
    names
}

#[test]
fn imports_the_default_branch_and_records_the_commit() -> TestResult {
    let mut f = fixture()?;
    let preview = f.library.preview(&source(&f, None, None)?)?;
    assert_eq!(names(&preview), ["root"]);
    // The preview names the branch git resolved.
    assert!(matches!(
        &preview.source,
        Source::Git { branch: Some(b), .. } if b == "main"
    ));
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    assert_eq!(clones_left(f.data.path())?, 1, "kept for the import");

    let report = f.library.import(&preview, &ids, &TagChoices::new())?;
    assert_eq!(report.imported.len(), 1);
    assert_eq!(clones_left(f.data.path())?, 0);

    let detail = f.library.detail(&report.imported[0])?;
    let record = detail.provenance.source;
    assert_eq!(record.kind, "git");
    assert_eq!(record.location, f.url);
    assert_eq!(record.branch.as_deref(), Some("main"));
    assert_eq!(record.subfolder, None);
    assert_eq!(record.commit_sha.as_deref(), Some(f.main_sha.as_str()));
    Ok(())
}

#[test]
fn an_explicit_branch_and_subfolder_are_scanned_and_recorded() -> TestResult {
    let mut f = fixture()?;
    let dev = f.library.preview(&source(&f, Some("dev"), None)?)?;
    assert_eq!(names(&dev), ["dev", "root"]);

    let preview = f.library.preview(&source(&f, Some("dev"), Some("pack"))?)?;
    assert_eq!(names(&preview), ["inner"]);
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    let report = f.library.import(&preview, &ids, &TagChoices::new())?;
    let record = f.library.detail(&report.imported[0])?.provenance.source;
    assert_eq!(record.branch.as_deref(), Some("dev"));
    assert_eq!(record.subfolder.as_deref(), Some("pack"));
    assert_eq!(record.commit_sha.as_deref(), Some(f.dev_sha.as_str()));
    // The stored copy holds the artifact's files, relative to the subfolder.
    assert_eq!(f.library.detail(&report.imported[0])?.files, ["inner.md"]);
    Ok(())
}

#[test]
fn a_missing_branch_is_reported_and_leaves_no_clone() {
    let f = fixture().expect("fixture");
    let source = source(&f, Some("nope"), None).expect("source");
    let err = f.library.preview(&source).expect_err("no such branch");
    assert!(
        matches!(&err, Error::GitBranchNotFound { branch } if branch == "nope"),
        "{err}"
    );
    assert_eq!(clones_left(f.data.path()).expect("list"), 0);
}

#[test]
fn a_missing_subfolder_is_reported_and_leaves_no_clone() {
    let f = fixture().expect("fixture");
    let source = source(&f, None, Some("absent/dir")).expect("source");
    let err = f.library.preview(&source).expect_err("no such folder");
    assert!(matches!(err, Error::GitSubfolderMissing { .. }), "{err}");
    assert_eq!(clones_left(f.data.path()).expect("list"), 0);
}

#[test]
fn a_subfolder_climbing_out_is_refused_before_cloning() {
    let f = fixture().expect("fixture");
    for sub in ["../x", "a/../../x"] {
        let source = Source::Git {
            url: f.url.clone(),
            branch: None,
            subfolder: Some(sub.to_owned()),
        };
        let err = f.library.preview(&source).expect_err("unsafe");
        assert!(matches!(err, Error::UnsafePath { .. }), "{err}");
    }
    assert_eq!(clones_left(f.data.path()).expect("list"), 0);
}

#[test]
fn a_repo_without_artifacts_is_reported_and_leaves_no_clone() -> TestResult {
    let f = fixture()?;
    let repo = tempfile::tempdir()?;
    git(repo.path(), &["init", "-q", "-b", "main"])?;
    write(repo.path(), "README.md", "hi\n")?;
    git(repo.path(), &["add", "-A"])?;
    git(repo.path(), &["commit", "-q", "-m", "x"])?;
    let source = Source::parse_git(&format!("file://{}", repo.path().display()), None, None)?;
    let err = f.library.preview(&source).expect_err("empty");
    assert!(matches!(err, Error::NothingFound { .. }), "{err}");
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[test]
fn a_repo_with_only_unplaced_files_is_previewed_for_claude() -> TestResult {
    let f = fixture()?;
    let repo = tempfile::tempdir()?;
    git(repo.path(), &["init", "-q", "-b", "main"])?;
    write(repo.path(), "prompts/review.md", "Review the diff.\n")?;
    git(repo.path(), &["add", "-A"])?;
    git(repo.path(), &["commit", "-q", "-m", "x"])?;
    let source = Source::parse_git(&format!("file://{}", repo.path().display()), None, None)?;
    let preview = f.library.preview(&source)?;
    assert!(preview.candidates.is_empty());
    assert_eq!(preview.unplaced, ["prompts/review.md"]);
    Ok(())
}

#[test]
fn an_unreachable_repo_is_reported_and_leaves_no_clone() -> TestResult {
    let f = fixture()?;
    let source = Source::parse_git("file:///definitely/not/here", None, None)?;
    let err = f.library.preview(&source).expect_err("no repo");
    assert!(matches!(err, Error::GitRepoUnreachable { .. }), "{err}");
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[test]
fn missing_git_is_reported_and_leaves_no_clone() -> TestResult {
    let mut f = fixture()?;
    let empty = tempfile::tempdir()?;
    f.library.git.path = Some(OsString::from(empty.path()));
    let err = f
        .library
        .preview(&source(&f, None, None)?)
        .expect_err("no git");
    assert!(matches!(err, Error::GitNotInstalled), "{err}");
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_slow_git_is_stopped_at_the_timeout_and_leaves_no_clone() -> TestResult {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture()?;
    let bin = tempfile::tempdir()?;
    let script = bin.path().join("slow-git");
    fs::write(&script, "#!/bin/sh\nexec sleep 30\n")?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    f.library.git.program = script.into_os_string();
    f.library.git.timeout = Duration::from_millis(200);
    let started = std::time::Instant::now();
    let err = f
        .library
        .preview(&source(&f, None, None)?)
        .expect_err("timeout");
    assert!(matches!(err, Error::GitTimeout { .. }), "{err}");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[test]
fn a_newer_git_preview_replaces_the_old_clone() -> TestResult {
    let mut f = fixture()?;
    let first = f.library.preview(&source(&f, None, None)?)?;
    let second = f.library.preview(&source(&f, Some("dev"), None)?)?;
    assert_eq!(clones_left(f.data.path())?, 1);
    // The first preview's clone is gone, so importing from it asks to preview again.
    let ids = vec![first.candidates[0].id.clone()];
    let err = f
        .library
        .import(&first, &ids, &TagChoices::new())
        .expect_err("expired");
    assert!(matches!(err, Error::PreviewExpired), "{err}");
    // That failed import must not have thrown the live clone away.
    assert_eq!(clones_left(f.data.path())?, 1);
    let ids: Vec<String> = second.candidates.iter().map(|c| c.id.clone()).collect();
    assert_eq!(
        f.library
            .import(&second, &ids, &TagChoices::new())?
            .imported
            .len(),
        2
    );
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[test]
fn dropping_the_library_removes_a_pending_clone() -> TestResult {
    let f = fixture()?;
    f.library.preview(&source(&f, None, None)?)?;
    assert_eq!(clones_left(f.data.path())?, 1);
    let Fixture { data, library, .. } = f;
    drop(library);
    assert_eq!(clones_left(data.path())?, 0);
    Ok(())
}

#[test]
fn a_bad_selection_still_cleans_up_the_clone() -> TestResult {
    let mut f = fixture()?;
    let preview = f.library.preview(&source(&f, None, None)?)?;
    let err = f
        .library
        .import(
            &preview,
            &["0000000000000000".to_owned()],
            &TagChoices::new(),
        )
        .expect_err("not in preview");
    assert!(matches!(err, Error::NotInPreview { .. }), "{err}");
    // Selection is checked first, so the clone is still there for a corrected retry.
    assert_eq!(clones_left(f.data.path())?, 1);
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

#[test]
fn importing_the_same_source_twice_marks_it_already_in_library() -> TestResult {
    let mut f = fixture()?;
    let preview = f.library.preview(&source(&f, None, None)?)?;
    let ids: Vec<String> = preview.candidates.iter().map(|c| c.id.clone()).collect();
    f.library.import(&preview, &ids, &TagChoices::new())?;
    let again = f.library.preview(&source(&f, None, None)?)?;
    assert!(again.candidates.iter().all(|c| c.already_in_library));
    let report = f.library.import(&again, &ids, &TagChoices::new())?;
    assert!(report.imported.is_empty());
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}

/// A git stand-in that records its argv and a few environment variables, then fails.
#[cfg(unix)]
fn recorder(bin: &Path, log: &Path) -> io::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let script = bin.join("recording-git");
    let body = format!(
        "#!/bin/sh\n{{ for a in \"$@\"; do echo \"ARG $a\"; done; env | grep '^GIT_' | sed 's/^/ENV /'; }} > '{}'\nexit 1\n",
        log.display()
    );
    fs::write(&script, body)?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
    Ok(script)
}

#[cfg(unix)]
#[test]
fn git_runs_with_a_scrubbed_environment_restricted_protocols_and_no_lfs_or_hooks() -> TestResult {
    let bin = tempfile::tempdir()?;
    let log = bin.path().join("log.txt");
    let config = GitConfig {
        program: recorder(bin.path(), &log)?.into_os_string(),
        ..GitConfig::default()
    };
    // Harmless in this process (other tests run git), but each must not reach the child.
    std::env::set_var("GIT_PROXY_COMMAND", "evil-proxy");
    std::env::set_var("GIT_TEMPLATE_DIR", "/nonexistent-template");
    std::env::set_var("GIT_CONFIG_KEY_9", "core.pager");
    std::env::set_var("GIT_CONFIG_VALUE_9", "evil");
    let parent = tempfile::tempdir()?;
    for call in 0..2 {
        let url = "https://example.test/o/r";
        let _ = if call == 0 {
            clone(&config, parent.path(), url, None).err()
        } else {
            ls_remote(&config, url, None).err()
        };
        let seen = fs::read_to_string(&log)?;
        for gone in [
            "GIT_PROXY_COMMAND",
            "GIT_TEMPLATE_DIR",
            "GIT_CONFIG_KEY_9",
            "GIT_CONFIG_VALUE_9",
        ] {
            assert!(!seen.contains(gone), "{gone} leaked: {seen}");
        }
        for want in [
            "ENV GIT_CONFIG_NOSYSTEM=1",
            "ENV GIT_LFS_SKIP_SMUDGE=1",
            "ARG protocol.allow=never",
            "ARG protocol.https.allow=always",
            "ARG protocol.ssh.allow=always",
            "ARG protocol.git.allow=always",
            "ARG filter.lfs.smudge=",
            "ARG filter.lfs.process=",
            "ARG core.hooksPath=/dev/null",
        ] {
            assert!(seen.contains(want), "missing {want}: {seen}");
        }
        assert!(
            !seen.contains("protocol.file.allow"),
            "file allowed for a remote URL: {seen}"
        );
    }
    // A user-typed local source may use the file protocol.
    let _ = ls_remote(&config, "file:///tmp/local", None);
    assert!(fs::read_to_string(&log)?.contains("ARG protocol.file.allow=always"));
    Ok(())
}

#[test]
fn an_oversized_clone_is_refused_and_deleted() -> TestResult {
    let mut f = fixture()?;
    f.library.git.max_checkout_bytes = 10;
    let err = f
        .library
        .preview(&source(&f, None, None)?)
        .expect_err("too big");
    assert!(err.to_string().contains("larger than"), "{err}");
    assert_eq!(clones_left(f.data.path())?, 0);
    Ok(())
}
