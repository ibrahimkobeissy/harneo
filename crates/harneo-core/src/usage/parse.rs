//! One JSONL log line to the names it used. Everything else in the line is dropped here, so no
//! message text ever reaches the index. The log format is Claude's, not ours: unknown lines and
//! fields are ignored, never an error.

use serde_json::Value;

use crate::artifact::ArtifactKind;

/// Longest name kept. A real skill, agent, command or server name is short; a longer value is
/// not a name and could carry text, so it is dropped.
const MAX_NAME: usize = 200;

/// One thing a session used, on one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Use {
    pub session_id: String,
    /// `YYYY-MM-DD`, from the line's timestamp.
    pub day: String,
    /// The line's `cwd`, empty when it has none.
    pub project_path: String,
    pub kind: ArtifactKind,
    pub name: String,
}

/// Whether a line is worth parsing: only tool calls and slash commands can hold a use. Checked
/// as a substring first so the bulk of a log (plain messages) is never parsed.
fn may_hold_use(line: &str) -> bool {
    line.contains("\"tool_use\"") || line.contains("<command-name>")
}

/// A line that is not shaped like a JSON object (cheap structural check, used to count broken
/// lines without parsing every one).
pub(crate) fn is_broken(line: &str) -> bool {
    let t = line.trim();
    !(t.starts_with('{') && t.ends_with('}'))
}

/// Pulls the names of what a log line used. `fallback_session` stands in for a missing
/// `sessionId`. A line that does not parse, or has no usable day, yields nothing.
pub(crate) fn uses_in_line(line: &str, fallback_session: &str) -> Vec<Use> {
    if !may_hold_use(line) {
        return Vec::new();
    }
    match serde_json::from_str::<Value>(line) {
        Ok(value) => uses_in(&value, fallback_session),
        Err(_) => Vec::new(),
    }
}

/// The uses in one parsed line.
pub(crate) fn uses_in(line: &Value, fallback_session: &str) -> Vec<Use> {
    let Some(day) = line
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(day_of)
    else {
        return Vec::new();
    };
    let session_id = line
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= MAX_NAME)
        .unwrap_or(fallback_session)
        .to_string();
    let project_path = line
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut names: Vec<(ArtifactKind, String)> = Vec::new();
    let content = line.pointer("/message/content");
    match line.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            for item in content.and_then(Value::as_array).into_iter().flatten() {
                tool_use(item, &mut names);
            }
        }
        Some("user") => command_uses(content, &mut names),
        _ => {}
    }
    names
        .into_iter()
        .map(|(kind, name)| Use {
            session_id: session_id.clone(),
            day: day.clone(),
            project_path: project_path.clone(),
            kind,
            name,
        })
        .collect()
}

/// `YYYY-MM-DD` from an ISO timestamp, if it looks like one.
fn day_of(timestamp: &str) -> Option<String> {
    let day = timestamp.get(..10)?;
    let b = day.as_bytes();
    let ok = b.iter().enumerate().all(|(i, c)| match i {
        4 | 7 => *c == b'-',
        _ => c.is_ascii_digit(),
    });
    ok.then(|| day.to_string())
}

fn clean(name: &str) -> Option<&str> {
    let name = name.trim();
    let ok = !name.is_empty()
        && name.len() <= MAX_NAME
        && !name.chars().any(|c| c.is_whitespace() || c.is_control());
    ok.then_some(name)
}

fn push_named(kind: ArtifactKind, name: &str, out: &mut Vec<(ArtifactKind, String)>) {
    let Some(name) = clean(name) else { return };
    out.push((kind, name.to_string()));
    if let Some(plugin) = crate::plugin::namespaced_plugin(name) {
        out.push((ArtifactKind::Plugin, plugin.to_string()));
    }
}

fn tool_use(item: &Value, out: &mut Vec<(ArtifactKind, String)>) {
    if item.get("type").and_then(Value::as_str) != Some("tool_use") {
        return;
    }
    let Some(tool) = item.get("name").and_then(Value::as_str) else {
        return;
    };
    let input_str = |key: &str| {
        item.pointer(&format!("/input/{key}"))
            .and_then(Value::as_str)
    };
    match tool {
        "Skill" => {
            if let Some(skill) = input_str("skill") {
                push_named(ArtifactKind::Skill, skill, out);
            }
        }
        "Task" | "Agent" => {
            if let Some(agent) = input_str("subagent_type") {
                push_named(ArtifactKind::Agent, agent, out);
            }
        }
        _ => {
            if let Some(rest) = tool.strip_prefix("mcp__") {
                if let Some(server) = rest.split("__").next().and_then(clean) {
                    out.push((ArtifactKind::Mcp, server.to_string()));
                }
            }
        }
    }
}

/// Slash commands in a user message: `<command-name>/name</command-name>`.
fn command_uses(content: Option<&Value>, out: &mut Vec<(ArtifactKind, String)>) {
    const OPEN: &str = "<command-name>";
    const CLOSE: &str = "</command-name>";
    let mut texts: Vec<&str> = Vec::new();
    match content {
        Some(Value::String(s)) => texts.push(s),
        Some(Value::Array(items)) => texts.extend(
            items
                .iter()
                .filter_map(|i| i.get("text").and_then(Value::as_str)),
        ),
        _ => {}
    }
    for text in texts {
        let mut rest = text;
        while let Some(start) = rest.find(OPEN) {
            let after = &rest[start + OPEN.len()..];
            let Some(end) = after.find(CLOSE) else { break };
            let name = after[..end].trim().trim_start_matches('/');
            push_named(ArtifactKind::Command, name, out);
            rest = &after[end + CLOSE.len()..];
        }
    }
}
