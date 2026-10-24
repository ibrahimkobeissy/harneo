use std::fs;

use super::*;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn library_with(
    files: &[(&str, &[u8])],
) -> std::result::Result<(tempfile::TempDir, Library), Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let library = Library::open(data.path())?;
    let folder = library.store.artifacts_dir().join("abc123");
    for (name, bytes) in files {
        let path = folder.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    }
    Ok((data, library))
}

#[test]
fn reads_text_files_including_nested_ones() -> TestResult {
    let (_d, lib) = library_with(&[("SKILL.md", b"# Hi"), ("scripts/a.py", b"print(1)")])?;
    assert_eq!(
        lib.read_artifact_file("abc123", "SKILL.md")?,
        FileView::Text {
            text: "# Hi".into()
        }
    );
    assert_eq!(
        lib.read_artifact_file("abc123", "scripts/a.py")?,
        FileView::Text {
            text: "print(1)".into()
        }
    );
    Ok(())
}

#[test]
fn reports_binary_and_oversize_files() -> TestResult {
    let big = vec![b'a'; (MAX_VIEW_BYTES + 1) as usize];
    let (_d, lib) = library_with(&[("b.bin", &[0xff, 0xfe, 0x00]), ("big.txt", &big)])?;
    assert_eq!(lib.read_artifact_file("abc123", "b.bin")?, FileView::Binary);
    assert!(matches!(
        lib.read_artifact_file("abc123", "big.txt")?,
        FileView::TooLarge { .. }
    ));
    Ok(())
}

#[test]
fn a_file_of_exactly_the_cap_is_shown() -> TestResult {
    let at_cap = vec![b'a'; MAX_VIEW_BYTES as usize];
    let (_d, lib) = library_with(&[("cap.txt", &at_cap)])?;
    assert!(matches!(
        lib.read_artifact_file("abc123", "cap.txt")?,
        FileView::Text { .. }
    ));
    Ok(())
}

#[test]
fn refuses_paths_that_leave_the_copy() -> TestResult {
    let (d, lib) = library_with(&[("SKILL.md", b"x")])?;
    fs::write(d.path().join("outside.txt"), "secret")?;
    for bad in [
        "../x",
        "../../outside.txt",
        "a/../SKILL.md",
        "/etc/hostname",
        "",
        ".",
        "a\\b",
    ] {
        assert!(
            lib.artifact_file("abc123", bad).is_err(),
            "{bad} should be refused"
        );
    }
    for bad_id in ["..", "a/b", "", "/abs"] {
        assert!(lib.artifact_file(bad_id, "SKILL.md").is_err(), "{bad_id}");
    }
    Ok(())
}

#[test]
fn refuses_a_directory() -> TestResult {
    let (_d, lib) = library_with(&[("scripts/a.py", b"x")])?;
    assert!(matches!(
        lib.artifact_file("abc123", "scripts"),
        Err(Error::UnsafePath { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn refuses_a_symlink_leaving_the_copy() -> TestResult {
    let (d, lib) = library_with(&[("SKILL.md", b"x")])?;
    let outside = d.path().join("outside.txt");
    fs::write(&outside, "secret")?;
    let folder = lib.store.artifacts_dir().join("abc123");
    std::os::unix::fs::symlink(&outside, folder.join("link.txt"))?;
    std::os::unix::fs::symlink(d.path(), folder.join("dir"))?;
    assert!(matches!(
        lib.read_artifact_file("abc123", "link.txt"),
        Err(Error::UnsafePath { .. })
    ));
    assert!(lib.artifact_file("abc123", "dir/outside.txt").is_err());
    Ok(())
}

#[test]
fn scripts_and_executables_may_run_when_opened_and_documents_do_not() -> TestResult {
    let dir = tempfile::tempdir()?;
    for name in [
        "run.sh",
        "Setup.BAT",
        "x.ps1",
        "tool.py",
        "a.command",
        "app.exe",
        "i.js",
    ] {
        let path = dir.path().join(name);
        fs::write(&path, b"x")?;
        assert!(may_run_when_opened(&path), "{name}");
    }
    for name in ["SKILL.md", "notes.txt", "data.json", "conf.toml"] {
        let path = dir.path().join(name);
        fs::write(&path, b"x")?;
        assert!(!may_run_when_opened(&path), "{name}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.path().join("README");
        fs::write(&path, b"x")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        assert!(
            may_run_when_opened(&path),
            "an executable file without an extension"
        );
    }
    Ok(())
}
