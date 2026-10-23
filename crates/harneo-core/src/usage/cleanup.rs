//! Cleanup suggestions and prune. The groups come from the same SQL as the library's Status, so
//! the numbers match the Library screen; prune takes an artifact out of the profiles that
//! install it and switches the projects on those profiles, through the install engine.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{last_used_sql, status_sql};
use crate::artifact::ArtifactKind;
use crate::error::{Error, Result};
use crate::install::{ApplyReport, Plan};
use crate::library::Library;
use crate::profiles::{ProfileId, ProfileSpec, ProfileUse};

/// Profiles an artifact in at least this many current versions counts as repeated.
pub const REPEATED_IN: usize = 3;

/// One artifact in a cleanup group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct CleanupItem {
    pub id: String,
    pub kind: ArtifactKind,
    pub name: String,
    pub trust_score: Option<u8>,
    /// Audit findings stored on the copy.
    pub finding_count: u32,
    /// The version it is on and the one its source has (outdated artifacts).
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    /// The library copy was edited since import.
    pub local_edits: bool,
    /// The most recent day (`YYYY-MM-DD`) it was used, over everything indexed.
    pub last_used: Option<String>,
    /// The profiles whose current version installs it.
    pub profiles: Vec<ProfileUse>,
}

/// Library artifacts that share a kind and a name under different ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Duplicate {
    pub name: String,
    pub kind: ArtifactKind,
    pub ids: Vec<String>,
}

/// What Cleanup suggests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Cleanup {
    /// Status Findings: audit findings and a trust score under 60.
    pub findings: Vec<CleanupItem>,
    /// Status Outdated.
    pub outdated: Vec<CleanupItem>,
    /// Status Unused (with the profiles that install each).
    pub unused: Vec<CleanupItem>,
    /// Artifacts installed by at least three profiles' current versions.
    pub repeated: Vec<CleanupItem>,
    pub duplicates: Vec<Duplicate>,
}

/// A profile prune saves a new version of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PruneProfile {
    pub id: ProfileId,
    pub name: String,
    pub from_version: u32,
    pub to_version: u32,
}

/// A project prune switches to a profile's new version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PruneProject {
    pub id: i64,
    pub name: String,
    pub profile_id: ProfileId,
    pub plan: Plan,
}

/// A project on a changed profile that stays as it is, because it is pinned to an older version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct NotSwitched {
    pub id: i64,
    pub name: String,
    pub profile_id: ProfileId,
    pub profile_version: u32,
}

/// What removing an artifact from its profiles would do. Nothing is saved by planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PrunePlan {
    pub artifact: String,
    pub artifact_name: String,
    pub profiles: Vec<PruneProfile>,
    /// Projects registered on those profiles at their current version.
    pub projects: Vec<PruneProject>,
    pub not_switched: Vec<NotSwitched>,
}

/// A project switched by prune.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PruneSwitched {
    pub project_id: i64,
    pub name: String,
    pub report: ApplyReport,
}

/// The step prune stopped at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PruneFailure {
    /// The profile or project that failed, by name.
    pub subject: String,
    pub message: String,
}

/// What prune did. When `failed` is set, everything listed was done and nothing after it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PruneReport {
    /// Profiles that got a new version.
    pub profiles: Vec<PruneProfile>,
    pub switched: Vec<PruneSwitched>,
    pub failed: Option<PruneFailure>,
    pub not_switched: Vec<NotSwitched>,
}

/// The spec without `artifact`: not picked, not the Settings template, not an MCP server.
fn without(spec: &ProfileSpec, artifact: &str) -> ProfileSpec {
    let mut next = spec.clone();
    next.items.retain(|id| id != artifact);
    if next.settings.as_deref() == Some(artifact) {
        next.settings = None;
    }
    next.mcp.retain(|m| m.artifact != artifact);
    next.manual_only.remove(artifact);
    next
}

fn kind_of(name: String) -> Result<ArtifactKind> {
    Ok(serde_json::from_value(serde_json::Value::String(name))?)
}

impl Library {
    /// What to clean up: the artifacts whose status is Findings, Outdated or Unused, those in
    /// three or more profiles, and names that appear twice.
    pub fn cleanup(&self) -> Result<Cleanup> {
        let conn = &self.store.conn;
        let membership = self.profile_membership()?;
        let sql = format!(
            "SELECT id, kind, name, trust_score, finding_count, from_version, to_version, \
                    last_used, status \
             FROM (SELECT a.id AS id, a.kind AS kind, a.name AS name, a.trust_score AS trust_score, \
                          COALESCE(json_array_length(a.findings), 0) AS finding_count, \
                          up.from_version AS from_version, up.to_version AS to_version, \
                          {last_used} AS last_used, {status} AS status \
                   FROM artifacts a LEFT JOIN upstream up ON up.artifact_id = a.id) \
             ORDER BY name COLLATE NOCASE, id",
            last_used = last_used_sql("a"),
            status = status_sql("a"),
        );
        let rows = conn
            .prepare(&sql)?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<u8>>(3)?,
                    r.get::<_, u32>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, String>(8)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Cleanup {
            findings: Vec::new(),
            outdated: Vec::new(),
            unused: Vec::new(),
            repeated: Vec::new(),
            duplicates: Vec::new(),
        };
        let mut names: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for (id, kind, name, trust_score, finding_count, from, to, last_used, status) in rows {
            names
                .entry((kind.clone(), name.clone()))
                .or_default()
                .push(id.clone());
            let profiles = membership.get(&id).cloned().unwrap_or_default();
            let repeated = profiles.len() >= REPEATED_IN;
            let outdated = status == "outdated";
            let item = CleanupItem {
                local_edits: outdated && self.has_local_edits(&id)?,
                from_version: outdated.then_some(from).flatten(),
                to_version: outdated.then_some(to).flatten(),
                kind: kind_of(kind)?,
                id,
                name,
                trust_score,
                finding_count,
                last_used,
                profiles,
            };
            match status.as_str() {
                "findings" => out.findings.push(item.clone()),
                "outdated" => out.outdated.push(item.clone()),
                "unused" => out.unused.push(item.clone()),
                _ => {}
            }
            if repeated {
                out.repeated.push(item);
            }
        }
        for ((kind, name), ids) in names {
            if ids.len() > 1 {
                out.duplicates.push(Duplicate {
                    name,
                    kind: kind_of(kind)?,
                    ids,
                });
            }
        }
        Ok(out)
    }

    /// What taking `artifact` out of every profile that holds it would do: each such profile's
    /// next version (not saved), and each project on it at its current version with the plan
    /// that switches it. Projects pinned to an older version are listed apart and stay.
    pub fn plan_prune(&self, artifact: &str) -> Result<PrunePlan> {
        let name: Option<String> = self
            .store
            .conn
            .query_row(
                "SELECT name FROM artifacts WHERE id = ?1",
                params![artifact],
                |r| r.get(0),
            )
            .optional()?;
        let artifact_name = name.ok_or_else(|| Error::NotInLibrary {
            id: artifact.to_owned(),
        })?;
        let projects = self.projects()?;
        let mut plan = PrunePlan {
            artifact: artifact.to_owned(),
            artifact_name,
            profiles: Vec::new(),
            projects: Vec::new(),
            not_switched: Vec::new(),
        };
        for summary in self.profiles()? {
            let mut view = self.profile(summary.id, None)?;
            let next = without(&view.spec, artifact);
            if next == view.spec {
                continue; // not picked here (or only pulled in by a bundle)
            }
            let from_version = view.current_version;
            view.spec = next;
            view.version = from_version + 1;
            plan.profiles.push(PruneProfile {
                id: summary.id,
                name: summary.name.clone(),
                from_version,
                to_version: from_version + 1,
            });
            for project in projects.iter().filter(|p| p.profile_id == summary.id) {
                if project.profile_version == from_version {
                    plan.projects.push(PruneProject {
                        id: project.id,
                        name: project.name.clone(),
                        profile_id: summary.id,
                        plan: self.plan_switch_to(project.id, view.clone())?,
                    });
                } else {
                    plan.not_switched.push(NotSwitched {
                        id: project.id,
                        name: project.name.clone(),
                        profile_id: summary.id,
                        profile_version: project.profile_version,
                    });
                }
            }
        }
        Ok(plan)
    }

    /// Saves the new profile versions of [`Library::plan_prune`], then switches each project to
    /// its profile's new version (a backup of every changed file, as always). It stops at the
    /// first failure and reports what was done. The artifact stays in the library.
    pub fn apply_prune(&self, artifact: &str) -> Result<PruneReport> {
        let plan = self.plan_prune(artifact)?;
        let mut report = PruneReport {
            profiles: Vec::new(),
            switched: Vec::new(),
            failed: None,
            not_switched: plan.not_switched,
        };
        let mut saved: BTreeMap<ProfileId, u32> = BTreeMap::new();
        for profile in plan.profiles {
            let step = self
                .profile(profile.id, None)
                .and_then(|view| self.save_profile(profile.id, &without(&view.spec, artifact)));
            match step {
                Ok(version) => {
                    saved.insert(profile.id, version);
                    report.profiles.push(PruneProfile {
                        to_version: version,
                        ..profile
                    });
                }
                Err(e) => {
                    report.failed = Some(PruneFailure {
                        subject: profile.name,
                        message: e.to_string(),
                    });
                    return Ok(report);
                }
            }
        }
        for project in plan.projects {
            let version = saved.get(&project.profile_id).copied();
            let step = self
                .plan_switch(project.id, project.profile_id, version)
                .and_then(|plan| self.apply(&plan));
            match step {
                Ok(applied) => report.switched.push(PruneSwitched {
                    project_id: project.id,
                    name: project.name,
                    report: applied,
                }),
                Err(e) => {
                    report.failed = Some(PruneFailure {
                        subject: project.name,
                        message: e.to_string(),
                    });
                    return Ok(report);
                }
            }
        }
        Ok(report)
    }
}
