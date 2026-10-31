use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::copy::{ImportHooks, SkipReason};
use super::*;
use crate::artifact::ArtifactKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, rel: &str, contents: &str) -> io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

/// A source folder with an agent, a skill with a script, a settings file and an MCP server.
fn fixture(root: &Path) -> io::Result<()> {
    write(
        root,
        "agents/scout.md",
        "---\nname: scout\ndescription: Finds things\n---\nLook around.\n",
    )?;
    write(
        root,
        "skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Takes notes\n---\nWrite.\n",
    )?;
    write(root, "skills/notes/scripts/run.sh", "echo notes\n")?;
    write(
        root,
        ".mcp.json",
        r#"{"mcpServers":{"fs":{"command":"npx","args":["-y","fs-server"],"env":{"LEVEL":"debug"}}}}"#,
    )?;
    write(root, "CLAUDE.md", "# Rules\n")
}

/// Every file and symlink below `root`, with its bytes (or link target).
fn snapshot(root: &Path) -> io::Result<BTreeMap<String, Vec<u8>>> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Vec<u8>>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
            let kind = entry.file_type()?;
            if kind.is_dir() {
                walk(&entry.path(), &format!("{name}/"), out)?;
            } else if kind.is_symlink() {
                let target = fs::read_link(entry.path())?;
                out.insert(name, target.to_string_lossy().into_owned().into_bytes());
            } else {
                out.insert(name, fs::read(entry.path())?);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(root, "", &mut out)?;
    Ok(out)
}

struct Setup {
    _data: tempfile::TempDir,
    data: std::path::PathBuf,
    source_dir: tempfile::TempDir,
    library: Library,
    source: Source,
}

fn setup() -> std::result::Result<Setup, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let source_dir = tempfile::tempdir()?;
    fixture(source_dir.path())?;
    let library = Library::open(data.path())?;
    let source = Source::Local {
        path: source_dir.path().to_path_buf(),
    };
    Ok(Setup {
        data: data.path().to_path_buf(),
        _data: data,
        source_dir,
        library,
        source,
    })
}

fn all_ids(preview: &Preview) -> Vec<ArtifactId> {
    preview.candidates.iter().map(|c| c.id.clone()).collect()
}

fn row_count(library: &Library, table: &str) -> i64 {
    library
        .store
        .conn
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(-1)
}

/// What is under `library/`: the db files aside, nothing but `artifacts/` may exist.
fn library_entries(data: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(data.join("library"))?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<_>>()?;
    names.retain(|n| !n.starts_with("library.db"));
    names.sort();
    Ok(names)
}

fn stored_folders(data: &Path) -> io::Result<usize> {
    Ok(fs::read_dir(data.join("library/artifacts"))?.count())
}

#[test]
fn preview_then_import_stores_the_ticked_artifacts_with_provenance() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    assert_eq!(preview.candidates.len(), 4);
    assert!(preview.candidates.iter().all(|c| !c.already_in_library));

    let skill = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Skill)
        .ok_or("skill")?;
    let agent = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Agent)
        .ok_or("agent")?;
    let report = s.library.import(
        &preview,
        &[skill.id.clone(), agent.id.clone()],
        &TagChoices::new(),
    )?;
    assert_eq!(report.imported, [skill.id.clone(), agent.id.clone()]);
    assert!(report.skipped_files.is_empty());
    assert_eq!(
        report.source_id,
        s.library.detail(&skill.id)?.provenance.source.id
    );
    assert_eq!(stored_folders(&s.data)?, 2);
    assert_eq!(row_count(&s.library, "artifacts"), 2);

    let detail = s.library.detail(&skill.id)?;
    assert_eq!(detail.artifact, skill.artifact);
    assert_eq!(detail.files, ["SKILL.md", "scripts/run.sh"]);
    assert_eq!(detail.provenance.source_path, "skills/notes");
    assert_eq!(detail.provenance.digest, skill.artifact.digest);
    assert!(detail.provenance.digest.is_some());
    assert!(detail.provenance.imported_at > 0);
    assert_eq!(detail.provenance.source.kind, "local");
    assert_eq!(
        Path::new(&detail.provenance.source.location),
        s.source_dir.path().canonicalize()?
    );
    assert!(detail.provenance.replaced_secret_keys.is_empty());
    assert_eq!(
        fs::read_to_string(
            s.data
                .join("library/artifacts")
                .join(&skill.id)
                .join("scripts/run.sh")
        )?,
        "echo notes\n"
    );
    assert_eq!(s.library.detail(&agent.id)?.files, ["scout.md"]);

    // A second preview marks what was imported, and only that.
    let again = s.library.preview(&s.source)?;
    for c in &again.candidates {
        assert_eq!(c.already_in_library, c.id == skill.id || c.id == agent.id);
    }
    Ok(())
}

#[test]
fn importing_everything_then_previewing_again_marks_all_already_in_library() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    s.library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    let again = s.library.preview(&s.source)?;
    assert!(again.candidates.iter().all(|c| c.already_in_library));
    // Importing them again changes nothing.
    let report = s
        .library
        .import(&again, &all_ids(&again), &TagChoices::new())?;
    assert!(report.imported.is_empty());
    assert_eq!(report.already_in_library.len(), 4);
    assert_eq!(row_count(&s.library, "artifacts"), 4);
    assert_eq!(row_count(&s.library, "sources"), 1);
    Ok(())
}

#[test]
fn an_mcp_server_is_stored_as_server_json() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let mcp = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Mcp)
        .ok_or("mcp")?;
    s.library
        .import(&preview, std::slice::from_ref(&mcp.id), &TagChoices::new())?;
    assert_eq!(s.library.detail(&mcp.id)?.files, ["server.json"]);
    let stored: serde_json::Value = serde_json::from_slice(&fs::read(
        s.data
            .join("library/artifacts")
            .join(&mcp.id)
            .join("server.json"),
    )?)?;
    assert_eq!(
        stored,
        serde_json::json!({"fs": {
            "command": "npx", "args": ["-y", "fs-server"], "env": {"LEVEL": "debug"}
        }})
    );
    Ok(())
}

/// Fails the nth file write, or the commit.
struct Fail {
    at_file: Option<usize>,
    at_commit: bool,
    files: AtomicUsize,
}

impl Fail {
    fn file(n: usize) -> Self {
        Self {
            at_file: Some(n),
            at_commit: false,
            files: AtomicUsize::new(0),
        }
    }
    fn commit() -> Self {
        Self {
            at_file: None,
            at_commit: true,
            files: AtomicUsize::new(0),
        }
    }
}

impl ImportHooks for Fail {
    fn before_file(&self, _id: &str, _rel: &str) -> io::Result<()> {
        if Some(self.files.fetch_add(1, Ordering::SeqCst)) == self.at_file {
            return Err(io::Error::other("injected copy failure"));
        }
        Ok(())
    }
    fn before_commit(&self) -> io::Result<()> {
        if self.at_commit {
            return Err(io::Error::other("injected commit failure"));
        }
        Ok(())
    }
}

#[test]
fn a_copy_failure_halfway_leaves_no_folder_and_no_row() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let before = snapshot(s.source_dir.path())?;
    // The fourth file write is part-way through the selection.
    let err = s
        .library
        .import_with(
            &preview,
            &all_ids(&preview),
            &TagChoices::new(),
            &Fail::file(3),
        )
        .expect_err("injected failure");
    assert!(err.to_string().contains("injected copy failure"));
    assert_eq!(stored_folders(&s.data)?, 0);
    assert_eq!(library_entries(&s.data)?, ["artifacts"]);
    assert_eq!(row_count(&s.library, "artifacts"), 0);
    assert_eq!(row_count(&s.library, "sources"), 0);
    assert_eq!(snapshot(s.source_dir.path())?, before);

    // The library still works afterwards.
    let report = s
        .library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    assert_eq!(report.imported.len(), 4);
    Ok(())
}

#[test]
fn a_commit_failure_removes_the_folders_already_in_place() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let err = s
        .library
        .import_with(
            &preview,
            &all_ids(&preview),
            &TagChoices::new(),
            &Fail::commit(),
        )
        .expect_err("injected failure");
    assert!(err.to_string().contains("injected commit failure"));
    assert_eq!(stored_folders(&s.data)?, 0);
    assert_eq!(library_entries(&s.data)?, ["artifacts"]);
    assert_eq!(row_count(&s.library, "artifacts"), 0);
    assert_eq!(row_count(&s.library, "sources"), 0);
    Ok(())
}

#[test]
fn a_leftover_folder_with_no_row_does_not_block_the_import() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let id = preview.candidates[0].id.clone();
    write(
        &s.data.join("library/artifacts"),
        &format!("{id}/stale.txt"),
        "x",
    )?;
    s.library
        .import(&preview, std::slice::from_ref(&id), &TagChoices::new())?;
    assert!(!s
        .library
        .detail(&id)?
        .files
        .contains(&"stale.txt".to_owned()));
    Ok(())
}

#[test]
fn remove_deletes_the_folder_and_the_rows() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    s.library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    let first = preview.candidates[0].id.clone();
    s.library.remove(&first)?;
    assert!(!s.data.join("library/artifacts").join(&first).exists());
    assert_eq!(row_count(&s.library, "artifacts"), 3);
    assert!(matches!(
        s.library.detail(&first),
        Err(Error::NotInLibrary { .. })
    ));
    assert_eq!(library_entries(&s.data)?, ["artifacts"]);
    // It can be imported again, and removing the rest drops the source row.
    assert!(!s.library.preview(&s.source)?.candidates[0].already_in_library);
    for c in &preview.candidates[1..] {
        s.library.remove(&c.id)?;
    }
    assert_eq!(row_count(&s.library, "artifacts"), 0);
    assert_eq!(row_count(&s.library, "sources"), 0);
    assert_eq!(stored_folders(&s.data)?, 0);
    Ok(())
}

#[test]
fn removing_an_unknown_artifact_is_an_error_and_changes_nothing() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    s.library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    assert!(matches!(
        s.library.remove("0000000000000000"),
        Err(Error::NotInLibrary { .. })
    ));
    assert_eq!(row_count(&s.library, "artifacts"), 4);
    assert_eq!(stored_folders(&s.data)?, 4);
    Ok(())
}

#[test]
fn the_source_folder_is_byte_identical_after_preview_and_import() -> TestResult {
    let mut s = setup()?;
    let before = snapshot(s.source_dir.path())?;
    let preview = s.library.preview(&s.source)?;
    s.library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    let again = s.library.preview(&s.source)?;
    s.library
        .import(&again, &all_ids(&again), &TagChoices::new())?;
    assert_eq!(snapshot(s.source_dir.path())?, before);
    Ok(())
}

#[test]
fn a_selected_id_must_be_in_the_preview() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let err = s
        .library
        .import(
            &preview,
            &["0123456789abcdef".to_owned()],
            &TagChoices::new(),
        )
        .expect_err("not in preview");
    assert!(matches!(err, Error::NotInPreview { .. }));

    // A candidate whose id does not match its source and path is refused too.
    let mut forged = preview.clone();
    forged.candidates[0].artifact.path = "agents/other.md".into();
    let id = forged.candidates[0].id.clone();
    assert!(matches!(
        s.library.import(&forged, &[id], &TagChoices::new()),
        Err(Error::NotInPreview { .. })
    ));
    assert_eq!(stored_folders(&s.data)?, 0);
    Ok(())
}

#[test]
fn an_unsafe_path_in_the_preview_is_refused_and_nothing_is_stored() -> TestResult {
    let mut s = setup()?;
    let mut preview = s.library.preview(&s.source)?;
    let identity = preview.source.identity()?;
    let candidate = preview
        .candidates
        .iter_mut()
        .find(|c| c.artifact.kind == ArtifactKind::Agent)
        .ok_or("agent")?;
    candidate.artifact.files = vec!["../outside.md".into()];
    candidate.id = ids::artifact_id(&identity, &candidate.artifact.path);
    let id = candidate.id.clone();
    assert!(matches!(
        s.library.import(&preview, &[id], &TagChoices::new()),
        Err(Error::UnsafePath { .. })
    ));
    assert_eq!(stored_folders(&s.data)?, 0);
    assert_eq!(library_entries(&s.data)?, ["artifacts"]);
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_secret_and_local_files_are_skipped_and_reported() -> TestResult {
    let mut s = setup()?;
    let outside = tempfile::tempdir()?;
    write(outside.path(), "target.txt", "private")?;
    let root = s.source_dir.path();
    std::os::unix::fs::symlink(
        outside.path().join("target.txt"),
        root.join("skills/notes/linked.txt"),
    )?;
    write(root, "skills/notes/credentials.json", "{}")?;
    write(root, "skills/notes/settings.local.json", "{}")?;
    write(root, "skills/notes/big.txt", &"x".repeat(1024 * 1024 + 1))?;
    let before = snapshot(root)?;

    let preview = s.library.preview(&s.source)?;
    let skill = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Skill)
        .ok_or("skill")?;
    let report = s.library.import(
        &preview,
        std::slice::from_ref(&skill.id),
        &TagChoices::new(),
    )?;

    let mut skipped: Vec<(&str, SkipReason)> = report
        .skipped_files
        .iter()
        .map(|f| (f.file.as_str(), f.reason))
        .collect();
    skipped.sort_by_key(|(file, _)| *file);
    assert_eq!(
        skipped,
        [
            ("skills/notes/big.txt", SkipReason::TooLarge),
            ("skills/notes/credentials.json", SkipReason::SecretFile),
            ("skills/notes/linked.txt", SkipReason::NotRegularFile),
            ("skills/notes/settings.local.json", SkipReason::LocalFile),
        ]
    );
    assert_eq!(
        s.library.detail(&skill.id)?.files,
        ["SKILL.md", "scripts/run.sh"]
    );
    assert_eq!(snapshot(root)?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_symlink_after_the_scan_is_refused() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let outside = tempfile::tempdir()?;
    write(outside.path(), "scripts/run.sh", "echo stolen")?;
    let scripts = s.source_dir.path().join("skills/notes/scripts");
    fs::remove_dir_all(&scripts)?;
    std::os::unix::fs::symlink(outside.path().join("scripts"), &scripts)?;
    let skill = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Skill)
        .ok_or("skill")?;
    assert!(matches!(
        s.library.import(
            &preview,
            std::slice::from_ref(&skill.id),
            &TagChoices::new()
        ),
        Err(Error::UnsafePath { .. })
    ));
    assert_eq!(stored_folders(&s.data)?, 0);
    Ok(())
}

#[test]
fn previewing_a_missing_folder_is_an_error() -> TestResult {
    let s = setup()?;
    let missing = Source::Local {
        path: s.source_dir.path().join("nope"),
    };
    assert!(s.library.preview(&missing).is_err());
    Ok(())
}

#[test]
fn preview_serialises_with_a_tagged_source() -> TestResult {
    let s = setup()?;
    let preview = s.library.preview(&s.source)?;
    let json = serde_json::to_value(&preview)?;
    assert_eq!(json["source"]["kind"], "local");
    assert_eq!(json["candidates"][0]["alreadyInLibrary"], false);
    let back: Preview = serde_json::from_value(json)?;
    assert_eq!(back, preview);
    Ok(())
}

#[test]
fn remove_checks_the_id_before_touching_the_disk() -> TestResult {
    let mut s = setup()?;
    let preview = s.library.preview(&s.source)?;
    s.library
        .import(&preview, &all_ids(&preview), &TagChoices::new())?;
    let before = stored_folders(&s.data)?;
    // A traversal id would rename a folder outside the artifacts dir if it reached the disk.
    for bad in ["..", "../x", "", "ABCDEF0123456789", "00000000000000000"] {
        assert!(matches!(
            s.library.remove(bad),
            Err(Error::NotInLibrary { .. })
        ));
    }
    assert_eq!(stored_folders(&s.data)?, before);
    Ok(())
}

/// A skill with a 2 MiB binary and a file just over the large-file ceiling (sparse, never read).
fn with_large_files(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root.join("skills/notes/assets"))?;
    fs::write(
        root.join("skills/notes/assets/song.mp3"),
        vec![0u8; 2 * 1024 * 1024],
    )?;
    fs::File::create(root.join("skills/notes/assets/huge.bin"))?.set_len(LARGE_FILE_CEILING + 1)
}

#[test]
fn large_files_are_skipped_unless_the_import_opts_in() -> TestResult {
    let mut s = setup()?;
    with_large_files(s.source_dir.path())?;
    let preview = s.library.preview(&s.source)?;
    let skill = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Skill)
        .ok_or("skill")?;
    // The preview already says which files the audit cannot read.
    assert_eq!(
        skill.audit.unaudited,
        [
            "skills/notes/assets/huge.bin",
            "skills/notes/assets/song.mp3"
        ]
    );
    let report = s.library.import(
        &preview,
        std::slice::from_ref(&skill.id),
        &TagChoices::new(),
    )?;
    let skipped: Vec<&str> = report
        .skipped_files
        .iter()
        .map(|f| f.file.as_str())
        .collect();
    assert_eq!(
        skipped,
        [
            "skills/notes/assets/huge.bin",
            "skills/notes/assets/song.mp3"
        ]
    );
    let (audit, _) = records::load_audit(&s.library.store.conn, &skill.id)?.ok_or("audit")?;
    assert!(audit.unaudited.is_empty(), "nothing large was copied");
    Ok(())
}

#[test]
fn opting_in_copies_large_files_up_to_the_ceiling_marks_them_unaudited_and_remembers_it(
) -> TestResult {
    let mut s = setup()?;
    with_large_files(s.source_dir.path())?;
    let preview = s.library.preview(&s.source)?;
    let skill = preview
        .candidates
        .iter()
        .find(|c| c.artifact.kind == ArtifactKind::Skill)
        .ok_or("skill")?;
    let report = s.library.import_with_options(
        &preview,
        std::slice::from_ref(&skill.id),
        &TagChoices::new(),
        &[],
        true,
    )?;
    let skipped: Vec<(&str, SkipReason)> = report
        .skipped_files
        .iter()
        .map(|f| (f.file.as_str(), f.reason))
        .collect();
    assert_eq!(
        skipped,
        [("skills/notes/assets/huge.bin", SkipReason::TooLarge)]
    );
    assert!(s
        .library
        .detail(&skill.id)?
        .files
        .contains(&"assets/song.mp3".to_owned()));
    let (audit, _) = records::load_audit(&s.library.store.conn, &skill.id)?.ok_or("audit")?;
    assert_eq!(audit.unaudited, ["skills/notes/assets/song.mp3"]);
    let remembered: bool =
        s.library
            .store
            .conn
            .query_row("SELECT large_files FROM sources", [], |r| r.get(0))?;
    assert!(remembered, "updates of this source copy large files too");
    Ok(())
}
