//! Parent and child on the map: a child (a skill or subagent that only runs through another
//! artifact) sits in its parent's zone and moves with it. The relation is Claude's reading;
//! nothing here detects it, it only keeps what Claude or the user set sound.

use std::collections::{HashMap, HashSet};

use super::map::{MapDoc, Zone, MAX_ALSO_RUN_BY};
use crate::artifact::ArtifactKind;
use crate::library::ArtifactId;

/// Most nesting levels under a root: command, then skill, then subagent.
const MAX_DEPTH: usize = 2;

fn can_parent(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Command | ArtifactKind::Skill | ArtifactKind::Agent
    )
}

fn can_be_child(kind: ArtifactKind) -> bool {
    matches!(kind, ArtifactKind::Skill | ArtifactKind::Agent)
}

fn zone_key(zone: &Zone) -> String {
    match zone {
        Zone::Stage { id } => format!("stage:{id}"),
        Zone::Toolbelt => "toolbelt".to_owned(),
        Zone::Floor => "floor".to_owned(),
    }
}

impl MapDoc {
    fn parent_of(&self, artifact: &str) -> Option<&str> {
        self.placements
            .iter()
            .find(|p| p.artifact == artifact)
            .and_then(|p| p.parent.as_deref())
    }

    /// How many ancestors `artifact` has, stopping early on a loop.
    fn ancestors(&self, artifact: &str) -> usize {
        let mut count = 0;
        let mut at = artifact;
        while let Some(parent) = self.parent_of(at) {
            count += 1;
            if count > self.placements.len() {
                break;
            }
            at = parent;
        }
        count
    }

    /// How many levels of children hang below `artifact`.
    fn height(&self, artifact: &str, guard: usize) -> usize {
        if guard > self.placements.len() {
            return 0;
        }
        self.placements
            .iter()
            .filter(|p| p.parent.as_deref() == Some(artifact))
            .map(|p| 1 + self.height(&p.artifact, guard + 1))
            .max()
            .unwrap_or(0)
    }

    /// The artifacts below `artifact`, at any depth.
    pub(crate) fn descendants(&self, artifact: &str) -> Vec<ArtifactId> {
        let mut out: Vec<ArtifactId> = Vec::new();
        let mut queue = vec![artifact.to_owned()];
        while let Some(at) = queue.pop() {
            for p in &self.placements {
                if p.parent.as_deref() == Some(at.as_str()) && !out.contains(&p.artifact) {
                    out.push(p.artifact.clone());
                    queue.push(p.artifact.clone());
                }
            }
        }
        out
    }

    /// Makes every parent link sound, in answer order: a parent that is not placed, is the
    /// artifact itself, has the wrong kind, would close a cycle or would nest too deep is dropped
    /// (the later placement loses it); `also_run_by` is cleaned; each child takes its parent's
    /// zone; then every parent is followed by its children in the order of its zone. `kinds`
    /// holds the kind of every artifact of the profile.
    pub(crate) fn normalize_family(&mut self, kinds: &HashMap<ArtifactId, ArtifactKind>) {
        let asked: Vec<(usize, Option<ArtifactId>)> = self
            .placements
            .iter_mut()
            .enumerate()
            .map(|(i, p)| (i, p.parent.take()))
            .collect();
        let placed: HashSet<ArtifactId> =
            self.placements.iter().map(|p| p.artifact.clone()).collect();
        // Links are added one at a time on an empty set, so a cycle shows as the child already
        // being an ancestor of the parent.
        for (i, parent) in asked {
            let Some(parent) = parent else { continue };
            let child = self.placements[i].artifact.clone();
            let kinds_fit = kinds.get(&child).is_some_and(|k| can_be_child(*k))
                && kinds.get(&parent).is_some_and(|k| can_parent(*k));
            if parent == child || !placed.contains(&parent) || !kinds_fit {
                continue;
            }
            let mut at = parent.as_str();
            let mut cycle = false;
            while let Some(up) = self.parent_of(at) {
                if up == child {
                    cycle = true;
                    break;
                }
                at = up;
            }
            if cycle || self.ancestors(&parent) + 1 + self.height(&child, 0) > MAX_DEPTH {
                continue;
            }
            self.placements[i].parent = Some(parent);
        }
        for p in &mut self.placements {
            let mut seen: Vec<ArtifactId> = Vec::new();
            if p.parent.is_some() {
                for id in std::mem::take(&mut p.also_run_by) {
                    if id != p.artifact
                        && Some(&id) != p.parent.as_ref()
                        && kinds.contains_key(&id)
                        && !seen.contains(&id)
                    {
                        seen.push(id);
                    }
                }
                seen.truncate(MAX_ALSO_RUN_BY);
            }
            p.also_run_by = seen;
        }
        self.group_families();
    }

    /// Puts each child in its parent's zone, then numbers every zone so that a parent is
    /// directly followed by its children (and theirs).
    pub(crate) fn group_families(&mut self) {
        for _ in 0..=MAX_DEPTH {
            let zones: HashMap<ArtifactId, Zone> = self
                .placements
                .iter()
                .map(|p| (p.artifact.clone(), p.zone.clone()))
                .collect();
            for p in &mut self.placements {
                if let Some(zone) = p.parent.as_ref().and_then(|a| zones.get(a)) {
                    p.zone = zone.clone();
                }
            }
        }
        let mut rows: Vec<usize> = (0..self.placements.len()).collect();
        rows.sort_by_key(|&i| {
            (
                zone_key(&self.placements[i].zone),
                self.placements[i].order,
                i,
            )
        });
        let mut sequence: Vec<usize> = Vec::new();
        let mut done: HashSet<usize> = HashSet::new();
        for &i in &rows {
            if self.placements[i].parent.is_none() {
                self.push_family(i, &rows, &mut sequence, &mut done);
            }
        }
        let mut next: HashMap<String, u32> = HashMap::new();
        for i in sequence {
            let n = next.entry(zone_key(&self.placements[i].zone)).or_insert(0);
            self.placements[i].order = *n;
            *n += 1;
        }
    }

    fn push_family(
        &self,
        at: usize,
        rows: &[usize],
        sequence: &mut Vec<usize>,
        done: &mut HashSet<usize>,
    ) {
        if !done.insert(at) {
            return;
        }
        sequence.push(at);
        let me = self.placements[at].artifact.as_str();
        for &child in rows {
            if self.placements[child].parent.as_deref() == Some(me) {
                self.push_family(child, rows, sequence, done);
            }
        }
    }

    /// Turns every child of `artifact` into an ordinary placement (it stays where it is).
    pub(crate) fn free_children(&mut self, artifact: &str) {
        for p in &mut self.placements {
            if p.parent.as_deref() == Some(artifact) {
                p.parent = None;
                p.also_run_by.clear();
            }
        }
    }

    /// Reasons the parent links are not sound; empty when they are.
    pub(crate) fn family_problems(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        for p in &self.placements {
            let Some(parent) = p.parent.as_deref() else {
                if !p.also_run_by.is_empty() {
                    reasons.push(format!("{:?} has also-run-by but no parent", p.artifact));
                }
                continue;
            };
            let Some(owner) = self.placements.iter().find(|o| o.artifact == parent) else {
                reasons.push(format!(
                    "the parent {parent:?} of {:?} is not placed",
                    p.artifact
                ));
                continue;
            };
            if parent == p.artifact || owner.zone != p.zone {
                reasons.push(format!(
                    "{:?} is not in the zone of its parent {parent:?}",
                    p.artifact
                ));
            }
            if self.ancestors(&p.artifact) + self.height(&p.artifact, 0) > MAX_DEPTH {
                reasons.push(format!("{:?} is nested too deep", p.artifact));
            }
            if p.also_run_by.len() > MAX_ALSO_RUN_BY {
                reasons.push(format!(
                    "{:?} is also run by too many artifacts",
                    p.artifact
                ));
            }
        }
        reasons
    }
}
