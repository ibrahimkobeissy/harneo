//! `harneo profiles`: list the profiles in the library.

use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use harneo_core::library::Library;
use harneo_core::profiles::ProfileSummary;

/// Opens the library in the data folder (`HARNEO_DATA_DIR` or the default one).
pub fn open_library() -> anyhow::Result<(Library, PathBuf)> {
    let dir = harneo_core::paths::data_dir()?;
    let library = Library::open(&dir).context("cannot open the library")?;
    Ok((library, dir))
}

pub fn run(json: bool, out: &mut impl Write) -> anyhow::Result<()> {
    let (library, _) = open_library()?;
    let profiles = library.profiles()?;
    if json {
        let text =
            serde_json::to_string_pretty(&profiles).context("cannot serialize the profiles")?;
        writeln!(out, "{text}")?;
    } else {
        write!(out, "{}", render(&profiles))?;
    }
    Ok(())
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn render(profiles: &[ProfileSummary]) -> String {
    if profiles.is_empty() {
        return "No profiles yet. Create one in the app.\n".to_owned();
    }
    let mut out = String::new();
    for p in profiles {
        let artifacts: u32 = p.counts.iter().map(|c| c.count).sum();
        let _ = writeln!(
            out,
            "{} (id {}) v{}: {}, {}",
            p.name,
            p.id,
            p.version,
            plural(artifacts, "artifact", "artifacts"),
            plural(p.project_count, "project", "projects"),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use harneo_core::artifact::ArtifactKind;
    use harneo_core::library::KindCount;

    fn summary(count: u32, projects: u32) -> ProfileSummary {
        ProfileSummary {
            id: 3,
            name: "Web".into(),
            description: String::new(),
            version: 2,
            counts: vec![KindCount {
                kind: ArtifactKind::Skill,
                count,
            }],
            file_count: count,
            project_count: projects,
            updated_at: 0,
            category: harneo_core::harnmap::CategoryId::Software,
            category_note: None,
        }
    }

    #[test]
    fn render_uses_singular_and_plural() {
        assert_eq!(
            render(&[summary(1, 1)]),
            "Web (id 3) v2: 1 artifact, 1 project\n"
        );
        assert_eq!(
            render(&[summary(4, 0)]),
            "Web (id 3) v2: 4 artifacts, 0 projects\n"
        );
    }
}
