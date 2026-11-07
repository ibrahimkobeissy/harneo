//! What Harneo knows from the files alone: bundle links, frontmatter fields, hooks, rule paths.
//! Everything here is a fact; nothing is Claude's reading.

use std::collections::{BTreeMap, HashMap};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::artifact::ArtifactKind;
use crate::error::Result;
use crate::library::{records, ArtifactId, Library};
use crate::profiles::{ProfileId, ResolvedItem};

/// What sort of fact it is, so the UI can tag it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum FactKind {
    /// It calls, or is called by, another artifact of the profile.
    Bundle,
    /// A subagent's `skills` field.
    Skills,
    /// A subagent's `mcpServers` field.
    McpServers,
    /// A skill's or command's allowed `mcp__` tools.
    AllowedMcp,
    /// A skill only the user can start.
    ManualOnly,
    /// A rule's `paths`.
    RulePaths,
    /// The settings template's permission rules.
    Permissions,
    /// A hook of the settings template.
    Hook,
    /// The settings template's status line.
    StatusLine,
    /// CLAUDE.md: loaded in every session.
    AlwaysLoaded,
    /// An output style: applies to every reply.
    OutputStyle,
    /// It names an MCP server the profile doesn't include.
    MissingMcp { server: String },
}

/// One fact, in short plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Fact {
    pub kind: FactKind,
    pub text: String,
}

/// The facts of each artifact that has any.
pub type Facts = BTreeMap<ArtifactId, Vec<Fact>>;

fn fact(kind: FactKind, text: impl Into<String>) -> Fact {
    Fact {
        kind,
        text: text.into(),
    }
}

/// "a", "a and b", "a, b and c".
pub(super) fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// A frontmatter list written as a YAML list or as comma-separated text.
pub(super) fn list_field(metadata: &Map<String, Value>, key: &str) -> Vec<String> {
    let items: Vec<String> = match metadata.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(s)) => s
            .split([',', ' '])
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    items
        .into_iter()
        .map(|i| i.trim().to_owned())
        .filter(|i| !i.is_empty())
        .collect()
}

/// How Claude Code spells a server inside a tool name.
pub(super) fn normalise(server: &str) -> String {
    server
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The server of a tool name `mcp__<server>__<tool>`.
pub(super) fn server_of(tool: &str) -> Option<&str> {
    let rest = tool.strip_prefix("mcp__")?;
    let server = rest.split("__").next()?;
    (!server.is_empty()).then_some(server)
}

fn count_rules(settings: &Map<String, Value>, list: &str) -> usize {
    settings
        .get("permissions")
        .and_then(|p| p.get(list))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

pub(super) fn rules_text(n: usize, list: &str) -> Option<String> {
    (n > 0).then(|| format!("{n} {list} rule{}", if n == 1 { "" } else { "s" }))
}

/// The command a hook or status line runs, as written (exec form joined with its arguments).
fn command_of(v: &Value) -> Option<String> {
    let Value::String(c) = v.get("command")? else {
        return None;
    };
    let args = v
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let words: Vec<&str> = std::iter::once(c.as_str())
        .chain(args.filter_map(Value::as_str))
        .collect();
    Some(words.join(" "))
}

fn settings_facts(metadata: &Map<String, Value>, out: &mut Vec<Fact>) {
    let counts: Vec<String> = ["deny", "allow", "ask"]
        .iter()
        .filter_map(|l| rules_text(count_rules(metadata, l), l))
        .collect();
    if !counts.is_empty() {
        out.push(fact(
            FactKind::Permissions,
            format!("settings.json: {}", counts.join(" · ")),
        ));
    }
    if let Some(Value::Object(events)) = metadata.get("hooks") {
        for (event, groups) in events {
            for group in groups.as_array().into_iter().flatten() {
                let matcher = group
                    .get("matcher")
                    .and_then(Value::as_str)
                    .filter(|m| !m.is_empty());
                for command in group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(command_of)
                {
                    let at = match matcher {
                        Some(m) => format!("{event} · {m}"),
                        None => event.clone(),
                    };
                    out.push(fact(FactKind::Hook, format!("Hook: {at} → {command}")));
                }
            }
        }
    }
    if let Some(status) = metadata.get("statusLine") {
        out.push(fact(
            FactKind::StatusLine,
            match command_of(status) {
                Some(c) => format!("Status line: {c}"),
                None => "Status line".to_owned(),
            },
        ));
    }
}

/// The names of the profile's MCP servers, as written in tool names.
struct Profile<'a> {
    items: &'a [ResolvedItem],
    mcp: Vec<String>,
}

impl Profile<'_> {
    fn has_server(&self, server: &str) -> bool {
        let wanted = normalise(server);
        self.mcp.iter().any(|m| *m == wanted)
    }

    fn name_of(&self, id: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.as_str())
    }
}

fn missing(profile: &Profile, servers: &[String], out: &mut Vec<Fact>) {
    for server in servers {
        if !profile.has_server(server)
            && !out.iter().any(|f| {
                f.kind
                    == FactKind::MissingMcp {
                        server: server.clone(),
                    }
            })
        {
            out.push(fact(
                FactKind::MissingMcp {
                    server: server.clone(),
                },
                format!("Names the MCP server {server}, which this profile doesn't include"),
            ));
        }
    }
}

fn item_facts(conn: &Connection, profile: &Profile, item: &ResolvedItem) -> Result<Vec<Fact>> {
    let mut out = Vec::new();
    let Some(detail) = records::load(conn, &item.id)? else {
        return Ok(out);
    };
    let metadata = &detail.artifact.metadata;
    let names = |links: &[crate::bundles::LinkView]| -> Vec<String> {
        links
            .iter()
            .filter_map(|l| profile.name_of(&l.id).map(str::to_owned))
            .collect()
    };
    let calls = names(&detail.links.needs);
    if !calls.is_empty() {
        out.push(fact(
            FactKind::Bundle,
            format!("Calls {} (bundle)", join_names(&calls)),
        ));
    }
    let called_by = names(&detail.links.needed_by);
    if !called_by.is_empty() {
        out.push(fact(
            FactKind::Bundle,
            format!("Called by {} (bundle)", join_names(&called_by)),
        ));
    }
    match item.kind {
        ArtifactKind::Agent => {
            let skills = list_field(metadata, "skills");
            if !skills.is_empty() {
                out.push(fact(
                    FactKind::Skills,
                    format!("Subagent skills field: {}", skills.join(", ")),
                ));
            }
            let servers: Vec<String> = metadata
                .get("mcpServers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            if !servers.is_empty() {
                out.push(fact(
                    FactKind::McpServers,
                    format!("Subagent mcpServers field: {}", servers.join(", ")),
                ));
            }
            missing(profile, &servers, &mut out);
        }
        ArtifactKind::Skill | ArtifactKind::Command => {
            let tools: Vec<String> = list_field(metadata, "allowed-tools")
                .into_iter()
                .filter(|t| server_of(t).is_some())
                .collect();
            if !tools.is_empty() {
                out.push(fact(
                    FactKind::AllowedMcp,
                    format!("Allowed tools: {}", tools.join(", ")),
                ));
            }
            let servers: Vec<String> = tools
                .iter()
                .filter_map(|t| server_of(t))
                .map(str::to_owned)
                .collect();
            missing(profile, &servers, &mut out);
            let own = match metadata.get("disable-model-invocation") {
                Some(Value::Bool(on)) => *on,
                Some(Value::String(t)) => t == "true",
                _ => false,
            };
            if item.kind == ArtifactKind::Skill && (item.manual_only || own) {
                out.push(fact(
                    FactKind::ManualOnly,
                    "Manual only: disable-model-invocation is set",
                ));
            }
        }
        ArtifactKind::Rule => {
            let paths = list_field(metadata, "paths");
            if !paths.is_empty() {
                out.push(fact(
                    FactKind::RulePaths,
                    format!("Rule paths: {}", paths.join(", ")),
                ));
            }
        }
        ArtifactKind::Settings => settings_facts(metadata, &mut out),
        ArtifactKind::ClaudeMd => out.push(fact(
            FactKind::AlwaysLoaded,
            "CLAUDE.md: loaded in every session",
        )),
        ArtifactKind::OutputStyle => out.push(fact(
            FactKind::OutputStyle,
            "Output style: applies to every reply",
        )),
        _ => {}
    }
    Ok(out)
}

/// The facts of each of `items` (the profile's resolved artifacts), from the files only.
pub(crate) fn facts(conn: &Connection, items: &[ResolvedItem]) -> Result<Facts> {
    let profile = Profile {
        items,
        mcp: items
            .iter()
            .filter(|i| i.kind == ArtifactKind::Mcp)
            .map(|i| normalise(&i.name))
            .collect(),
    };
    let mut out = Facts::new();
    for item in items {
        let found = item_facts(conn, &profile, item)?;
        if !found.is_empty() {
            out.insert(item.id.clone(), found);
        }
    }
    Ok(out)
}

/// Groups of `items` joined by confirmed links, in profile order; a group has two or more
/// members.
pub(crate) fn bundles(conn: &Connection, items: &[ResolvedItem]) -> Result<Vec<Vec<ArtifactId>>> {
    let index: HashMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id.as_str(), i))
        .collect();
    let mut group: Vec<usize> = (0..items.len()).collect();
    fn root(group: &mut [usize], mut at: usize) -> usize {
        while group[at] != at {
            group[at] = group[group[at]];
            at = group[at];
        }
        at
    }
    for (from, item) in items.iter().enumerate() {
        for link in crate::bundles::links_of(conn, &item.id)?.needs {
            if let Some(&to) = index.get(link.id.as_str()) {
                let (a, b) = (root(&mut group, from), root(&mut group, to));
                group[a.max(b)] = a.min(b);
            }
        }
    }
    let mut by_root: BTreeMap<usize, Vec<ArtifactId>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let r = root(&mut group, i);
        by_root.entry(r).or_default().push(item.id.clone());
    }
    Ok(by_root.into_values().filter(|g| g.len() > 1).collect())
}

impl Library {
    /// The facts of every artifact the profile installs (its current version).
    pub fn harnmap_facts(&self, profile: ProfileId) -> Result<Facts> {
        let resolved = self.resolve(&self.profile(profile, None)?.spec)?;
        facts(&self.store.conn, &resolved.artifacts)
    }

    /// The groups of the profile's artifacts joined by confirmed links, for the shaded groups.
    pub fn harnmap_bundles(&self, profile: ProfileId) -> Result<Vec<Vec<ArtifactId>>> {
        let resolved = self.resolve(&self.profile(profile, None)?.spec)?;
        bundles(&self.store.conn, &resolved.artifacts)
    }
}
