//! Shared setup for the switch, removal and restore tests: installing over a project that has
//! files of its own, the way a refit does, and a second version of the fixture profile.

use std::fs;
use std::path::Path;

use super::fixture::Fx;
use super::{ApplyReport, Plan, PlanKind, WriteAction};
use crate::artifact::ArtifactKind;
use crate::profiles::ProfileSpec;

/// Every file under `root`, relative, sorted.
pub fn files_under(root: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The scaffold plan of `profile` rewritten as a refit of `dir`: `Replace` where the project has
/// the file, `New` where it does not.
pub fn refit_plan(
    f: &Fx,
    dir: &Path,
    profile: i64,
) -> std::result::Result<Plan, Box<dyn std::error::Error>> {
    let empty = tempfile::tempdir()?;
    let mut plan = f.library.plan_scaffold(empty.path(), profile, None)?;
    plan.target = dir.canonicalize()?.display().to_string();
    plan.kind = PlanKind::Refit;
    for write in &mut plan.writes {
        if dir.join(&write.path).exists() {
            write.action = WriteAction::Replace;
        }
    }
    plan.backup_count = u32::try_from(
        plan.writes
            .iter()
            .filter(|w| w.action == WriteAction::Replace)
            .count(),
    )?;
    Ok(plan)
}

/// Installs `profile` into `dir` like a refit that replaces every file of the project that the
/// profile also writes.
pub fn install_over(
    f: &Fx,
    dir: &Path,
    profile: i64,
) -> std::result::Result<ApplyReport, Box<dyn std::error::Error>> {
    let plan = refit_plan(f, dir, profile)?;
    Ok(f.library.apply(&plan)?)
}

/// A profile "mini" that holds the skill and the CLAUDE.md of the fixture profile. Saving a
/// second version with `with_agent` adds the agent.
pub fn mini_profile(f: &Fx) -> std::result::Result<i64, Box<dyn std::error::Error>> {
    let web = f.library.profile(f.profile, None)?;
    let skill = f.library.find(ArtifactKind::Skill, "s")?;
    let claude = f.library.find(ArtifactKind::ClaudeMd, "CLAUDE.md")?;
    let spec = ProfileSpec {
        items: skill.into_iter().chain(claude).collect(),
        ..web.spec
    };
    Ok(f.library.create_profile("mini", "", &spec)?)
}

/// Saves a new version of `profile` that also holds the agent `rev`; returns the version.
pub fn with_agent(f: &Fx, profile: i64) -> std::result::Result<u32, Box<dyn std::error::Error>> {
    let view = f.library.profile(profile, None)?;
    let agent = f
        .library
        .find(ArtifactKind::Agent, "rev")?
        .ok_or("agent missing")?;
    let mut spec = view.spec;
    spec.items.push(agent);
    Ok(f.library.save_profile(profile, &spec)?)
}

/// Saves a new version of the fixture profile without its agent; returns the version.
pub fn without_agent(f: &Fx) -> std::result::Result<u32, Box<dyn std::error::Error>> {
    let view = f.library.profile(f.profile, None)?;
    let agent = f
        .library
        .find(ArtifactKind::Agent, "rev")?
        .ok_or("agent missing")?;
    let mut spec = view.spec;
    spec.items.retain(|id| *id != agent);
    Ok(f.library.save_profile(f.profile, &spec)?)
}

pub fn read(dir: &Path, rel: &str) -> std::io::Result<Vec<u8>> {
    fs::read(dir.join(rel))
}
