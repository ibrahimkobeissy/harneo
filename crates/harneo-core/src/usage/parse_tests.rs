//! Synthetic log lines only, in the shapes `spec/usage.md` Decision 2 describes.

use serde_json::json;

use super::parse::{is_broken, uses_in_line, Use};
use crate::artifact::ArtifactKind;

/// Text that must never reach the index.
pub(crate) const SECRET_TEXT: &str = "distinctive-message-text-zq81";

pub(crate) fn assistant(session: &str, ts: &str, cwd: &str, items: serde_json::Value) -> String {
    json!({
        "type": "assistant", "sessionId": session, "timestamp": ts, "cwd": cwd,
        "message": { "role": "assistant", "content": items }
    })
    .to_string()
}

pub(crate) fn tool(name: &str, input: serde_json::Value) -> serde_json::Value {
    json!({ "type": "tool_use", "id": "t1", "name": name, "input": input })
}

pub(crate) fn user(session: &str, ts: &str, cwd: &str, text: &str) -> String {
    json!({
        "type": "user", "sessionId": session, "timestamp": ts, "cwd": cwd,
        "message": { "role": "user", "content": text }
    })
    .to_string()
}

fn one(line: &str) -> Vec<Use> {
    uses_in_line(line, "fallback")
}

fn pairs(uses: &[Use]) -> Vec<(ArtifactKind, &str)> {
    uses.iter().map(|u| (u.kind, u.name.as_str())).collect()
}

#[test]
fn a_skill_call_is_a_skill_use_with_session_day_and_project() {
    let line = assistant(
        "s1",
        "2026-10-20T12:00:00.000Z",
        "/work/app",
        json!([{ "type": "text", "text": SECRET_TEXT }, tool("Skill", json!({"skill": "webperf"}))]),
    );
    let uses = one(&line);
    assert_eq!(
        uses,
        vec![Use {
            session_id: "s1".into(),
            day: "2026-10-20".into(),
            project_path: "/work/app".into(),
            kind: ArtifactKind::Skill,
            name: "webperf".into(),
        }]
    );
}

#[test]
fn task_and_agent_calls_are_subagent_uses() {
    for tool_name in ["Task", "Agent"] {
        let line = assistant(
            "s",
            "2026-10-20T00:00:00Z",
            "",
            json!([tool(
                tool_name,
                json!({"subagent_type": "reviewer", "prompt": SECRET_TEXT})
            )]),
        );
        assert_eq!(pairs(&one(&line)), vec![(ArtifactKind::Agent, "reviewer")]);
    }
}

#[test]
fn mcp_tools_count_for_their_server() {
    let line = assistant(
        "s",
        "2026-10-20T00:00:00Z",
        "",
        json!([
            tool("mcp__github__create_issue", json!({})),
            tool("mcp__a__b__c", json!({}))
        ]),
    );
    assert_eq!(
        pairs(&one(&line)),
        vec![(ArtifactKind::Mcp, "github"), (ArtifactKind::Mcp, "a")]
    );
}

#[test]
fn a_command_in_a_user_message_is_a_command_use() {
    let text = format!(
        "<command-message>x</command-message><command-name>/review</command-name> {SECRET_TEXT}"
    );
    let line = user("s", "2026-10-20T00:00:00Z", "/p", &text);
    assert_eq!(pairs(&one(&line)), vec![(ArtifactKind::Command, "review")]);
    let blocks = json!({
        "type": "user", "sessionId": "s", "timestamp": "2026-10-20T00:00:00Z",
        "message": { "content": [{ "type": "text", "text": "<command-name>/a</command-name> and <command-name>/b</command-name>" }] }
    })
    .to_string();
    assert_eq!(
        pairs(&one(&blocks)),
        vec![(ArtifactKind::Command, "a"), (ArtifactKind::Command, "b")]
    );
}

#[test]
fn namespaced_names_also_count_for_their_plugin() {
    let line = assistant(
        "s",
        "2026-10-20T00:00:00Z",
        "",
        json!([
            tool("Skill", json!({"skill": "kit:lint"})),
            tool("Task", json!({"subagent_type": "kit:fixer"}))
        ]),
    );
    assert_eq!(
        pairs(&one(&line)),
        vec![
            (ArtifactKind::Skill, "kit:lint"),
            (ArtifactKind::Plugin, "kit"),
            (ArtifactKind::Agent, "kit:fixer"),
            (ArtifactKind::Plugin, "kit"),
        ]
    );
    let cmd = user(
        "s",
        "2026-10-20T00:00:00Z",
        "",
        "<command-name>/kit:ship</command-name>",
    );
    assert_eq!(
        pairs(&one(&cmd)),
        vec![
            (ArtifactKind::Command, "kit:ship"),
            (ArtifactKind::Plugin, "kit")
        ]
    );
}

#[test]
fn a_missing_session_id_falls_back_to_the_file_name() {
    let line = json!({
        "type": "assistant", "timestamp": "2026-10-20T00:00:00Z",
        "message": { "content": [tool("Skill", json!({"skill": "x"}))] }
    })
    .to_string();
    assert_eq!(one(&line)[0].session_id, "fallback");
}

#[test]
fn unknown_shapes_and_bad_values_yield_nothing() {
    let cases = [
        json!({"type": "summary", "summary": SECRET_TEXT}).to_string(),
        json!({"type": "assistant", "timestamp": "2026-10-20T00:00:00Z", "message": {"content": "tool_use"}}).to_string(),
        // no timestamp, bad timestamp
        assistant("s", "", "", json!([tool("Skill", json!({"skill": "x"}))])).replace("\"timestamp\":\"\"", "\"x\":1"),
        assistant("s", "yesterday", "", json!([tool("Skill", json!({"skill": "x"}))])),
        // skill input missing or not a string, name with spaces, overlong name
        assistant("s", "2026-10-20T00:00:00Z", "", json!([tool("Skill", json!({})), tool("Skill", json!({"skill": 4}))])),
        assistant("s", "2026-10-20T00:00:00Z", "", json!([tool("Skill", json!({"skill": "two words"}))])),
        assistant("s", "2026-10-20T00:00:00Z", "", json!([tool("Skill", json!({"skill": "x".repeat(300)}))])),
        // tool_use without a name, other tools, wrong item type
        assistant("s", "2026-10-20T00:00:00Z", "", json!([{"type": "tool_use"}, tool("Bash", json!({"command": "ls"})), {"type": "tool_result"}])),
        // command tag never closed, in an assistant message, or empty
        user("s", "2026-10-20T00:00:00Z", "", "<command-name>/oops"),
        user("s", "2026-10-20T00:00:00Z", "", "<command-name>/</command-name>"),
        json!({"type": "system", "timestamp": "2026-10-20T00:00:00Z", "message": {"content": "<command-name>/x</command-name>"}}).to_string(),
        json!({"type": "assistant", "timestamp": "2026-10-20T00:00:00Z", "sessionId": "s", "message": "tool_use"}).to_string(),
    ];
    for case in cases {
        assert!(one(&case).is_empty(), "{case}");
    }
}

#[test]
fn broken_json_is_flagged_and_yields_nothing() {
    assert!(is_broken("not json"));
    assert!(is_broken("{\"truncated\": "));
    assert!(!is_broken("  {\"a\":1}  "));
    assert!(one("{\"type\":\"assistant\" \"tool_use\"}").is_empty());
}

#[test]
fn lines_that_cannot_hold_a_use_are_not_parsed() {
    let line = json!({"type": "user", "timestamp": "2026-10-20T00:00:00Z", "message": {"content": SECRET_TEXT}}).to_string();
    assert!(one(&line).is_empty());
}
