//! A hostile folder, built at test time (no secret-looking file is ever committed): the scan must
//! finish, report each problem under its own code, and never read what it must not read.

use std::fs;
use std::path::Path;

use harneo_core::artifact::walk::resolve_reference;
use harneo_core::artifact::{AgentAdapter, Artifact, ClaudeAdapter, ScanOptions, ScanReport};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SENTINEL_ENV: &str = "HARNEO_SENTINEL_ENV_DO_NOT_READ";
const SENTINEL_KEY: &str = "HARNEO_SENTINEL_KEYFILE_DO_NOT_READ";
const SENTINEL_LOCAL: &str = "HARNEO_SENTINEL_LOCAL_DO_NOT_READ";
const SENTINEL_TARGET: &str = "HARNEO_SENTINEL_SYMLINK_TARGET_DO_NOT_READ";
const VALID: &str = "---\nname: NAME\ndescription: d\n---\nbody";

fn write(root: &Path, rel: &str, contents: &[u8]) -> std::io::Result<()> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}

fn skill(name: &str) -> String {
    VALID.replace("NAME", name)
}

/// Nine levels of nine aliases each: a billion-laughs document.
fn alias_bomb() -> String {
    let mut yaml =
        String::from("---\nname: bomb\ndescription: d\nl0: &l0 [x, x, x, x, x, x, x, x, x]\n");
    for i in 1..9 {
        let items = vec![format!("*l{}", i - 1); 9].join(", ");
        yaml.push_str(&format!("l{i}: &l{i} [{items}]\n"));
    }
    yaml.push_str("---\nbody");
    yaml
}

fn find<'a>(report: &'a ScanReport, path: &str) -> Result<&'a Artifact, String> {
    report
        .artifacts
        .iter()
        .find(|a| a.path == path)
        .ok_or_else(|| format!("no artifact at {path}"))
}

fn codes(a: &Artifact) -> Vec<&str> {
    a.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

/// Builds the hostile root; the symlink targets live in a separate folder outside it.
fn build(root: &Path, outside: &Path) -> TestResult {
    write(outside, "target.txt", SENTINEL_TARGET.as_bytes())?;

    write(root, "skills/evil/SKILL.md", skill("evil").as_bytes())?;
    write(root, "skills/evil/.env", SENTINEL_ENV.as_bytes())?;
    write(root, "skills/evil/deploy.key", SENTINEL_KEY.as_bytes())?;
    write(
        root,
        "skills/evil/settings.local.json",
        SENTINEL_LOCAL.as_bytes(),
    )?;
    write(root, "skills/evil/big.bin", &vec![0u8; 1024 * 1024 + 1])?;
    write(root, "skills/evil/node_modules/dep/index.js", b"skipped")?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            outside.join("target.txt"),
            root.join("skills/evil/link.txt"),
        )?;
        std::os::unix::fs::symlink(outside, root.join("skills/evil/linked-dir"))?;
    }

    write(root, "skills/bomb/SKILL.md", alias_bomb().as_bytes())?;

    let mut bad = b"---\nname: badutf\ndescription: d\n---\n".to_vec();
    bad.extend_from_slice(&[0xff, 0xfe, 0xfd]);
    write(root, "skills/badutf/SKILL.md", &bad)?;

    write(root, "skills/deep/SKILL.md", skill("deep").as_bytes())?;
    let nest = vec!["d"; 40].join("/");
    write(root, &format!("skills/deep/{nest}/leaf.md"), b"leaf")?;
    Ok(())
}

#[test]
fn hostile_folder_finishes_with_a_code_per_problem_and_leaks_nothing() -> TestResult {
    let root = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    build(root.path(), outside.path())?;

    let report = ClaudeAdapter.scan(root.path(), &ScanOptions::default())?;

    let evil = find(&report, "skills/evil")?;
    let evil_codes = codes(evil);
    for code in ["walk.secret-file", "walk.local-file", "walk.file-too-large"] {
        assert!(evil_codes.contains(&code), "{code} in {evil_codes:?}");
    }
    #[cfg(unix)]
    {
        assert_eq!(
            evil_codes.iter().filter(|c| **c == "walk.symlink").count(),
            2
        );
        assert!(evil.files.contains(&"skills/evil/link.txt".to_owned()));
        assert!(evil.files.contains(&"skills/evil/linked-dir".to_owned()));
    }
    let secret_files: Vec<_> = evil
        .diagnostics
        .iter()
        .filter(|d| d.code == "walk.secret-file")
        .map(|d| d.file.as_str())
        .collect();
    assert_eq!(secret_files, ["skills/evil/.env", "skills/evil/deploy.key"]);
    assert!(evil.files.contains(&"skills/evil/.env".to_owned()));
    assert!(!evil.files.iter().any(|f| f.contains("node_modules")));

    assert!(codes(find(&report, "skills/bomb")?).contains(&"skill.invalid-frontmatter"));
    assert_eq!(
        codes(find(&report, "skills/badutf")?),
        ["walk.invalid-utf8"]
    );
    assert!(codes(find(&report, "skills/deep")?).contains(&"walk.too-deep"));

    let json = serde_json::to_string(&report)?;
    for sentinel in [SENTINEL_ENV, SENTINEL_KEY, SENTINEL_LOCAL, SENTINEL_TARGET] {
        assert!(!json.contains(sentinel), "{sentinel} leaked");
        assert!(
            !format!("{report:?}").contains(sentinel),
            "{sentinel} leaked"
        );
    }
    // Paths in the report stay relative to the root.
    assert!(!json.contains(&root.path().display().to_string()));
    assert!(!json.contains(&outside.path().display().to_string()));
    Ok(())
}

#[test]
fn a_reference_leaving_the_root_is_reported_not_resolved() {
    let e = resolve_reference(Path::new("skills/evil"), "../../../outside/target.txt");
    let d = e.err();
    assert_eq!(
        d.as_ref().map(|d| d.code.as_str()),
        Some("walk.path-escapes-root")
    );
    assert_eq!(d.map(|d| d.file), Some("skills/evil".into()));
    assert!(resolve_reference(Path::new("skills/evil"), "../other/x.md").is_ok());
}

#[test]
fn limits_come_from_scan_options() -> TestResult {
    let root = tempfile::tempdir()?;
    write(root.path(), "skills/a/SKILL.md", skill("a").as_bytes())?;
    write(root.path(), "skills/b/SKILL.md", skill("b").as_bytes())?;
    write(root.path(), "skills/a/vendor/x.md", b"x")?;
    let size = skill("a").len();

    // Room for one SKILL.md only: the second is not read and a root diagnostic says so.
    let tight = ScanOptions {
        max_total_bytes: size + 1,
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(root.path(), &tight)?;
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].code, "walk.scan-limit-reached");
    assert_eq!(report.diagnostics[0].file, "");
    assert_eq!(find(&report, "skills/a")?.description.as_deref(), Some("d"));
    assert_eq!(find(&report, "skills/b")?.description, None);

    // A smaller per-file limit makes the same file too large.
    let small = ScanOptions {
        max_file_bytes: size - 1,
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(root.path(), &small)?;
    assert_eq!(codes(find(&report, "skills/a")?), ["walk.file-too-large"]);

    // skip_dirs is configurable.
    let skip = ScanOptions {
        skip_dirs: vec!["vendor".into()],
        ..ScanOptions::default()
    };
    let report = ClaudeAdapter.scan(root.path(), &skip)?;
    assert_eq!(find(&report, "skills/a")?.files, ["skills/a/SKILL.md"]);
    let report = ClaudeAdapter.scan(root.path(), &ScanOptions::default())?;
    assert_eq!(find(&report, "skills/a")?.files.len(), 2);
    Ok(())
}
