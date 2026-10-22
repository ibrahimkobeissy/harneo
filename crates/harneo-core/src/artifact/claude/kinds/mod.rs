//! One file per artifact type.

pub(super) mod agent;
pub(super) mod claude_md;
pub(super) mod command;
pub(super) mod mcp;
pub(super) mod output_style;
pub(super) mod plugin;
pub(super) mod rule;
pub(super) mod settings;
pub(super) mod skill;
pub(super) mod workflow;

/// What lies below the folder named `dir` in a path found in a project (`.claude/<dir>/x`) or a
/// pack (`<dir>/x`); the file name when the folder is not in the path.
pub(super) fn below(path: &str, dir: &str) -> std::path::PathBuf {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    let rest = match parts.iter().position(|p| *p == dir) {
        Some(i) if i + 1 < parts.len() => &parts[i + 1..],
        _ => &parts[parts.len().saturating_sub(1)..],
    };
    rest.iter().collect()
}
