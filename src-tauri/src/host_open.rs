//! Opens files, folders and web pages with the system's default apps.
//!
//! tauri-plugin-opener's `open` crate starts the launcher (`xdg-open`, `gio`, ...) with Harneo's
//! own environment. Inside an AppImage that environment points the dynamic loader and GTK at the
//! bundle, and the file manager or editor the launcher starts crashes on a symbol lookup error.
//! Here the launcher gets the host environment (`harneo_core::host_env::for_host`) instead.

use std::ffi::OsStr;
use std::io;
use std::process::{Command, Stdio};
use std::thread;

/// The web pages the window may open, by name; the window never passes a URL itself.
pub fn link_url(name: &str) -> Option<&'static str> {
    match name {
        "claudeInstall" => Some("https://code.claude.com"),
        "errorReporting" => Some(
            "https://github.com/ibrahimkobeissy/harneo/blob/main/BUILDING.md#error-reporting-optional",
        ),
        _ => None,
    }
}

/// Opens `target` (a path or URL) with its default app, through the first launcher installed.
pub fn open(target: impl AsRef<OsStr>) -> io::Result<()> {
    spawn_first(::open::commands(target))
}

/// Starts the first of `commands` that is installed, with the host environment, and returns
/// once it has started. A launcher may wait for the app it starts, so it is reaped on a thread.
fn spawn_first(commands: Vec<Command>) -> io::Result<()> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "no program to open it with");
    for mut command in commands {
        harneo_core::host_env::for_host(&mut command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match command.spawn() {
            Ok(mut child) => {
                thread::spawn(move || child.wait());
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => last = e,
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_links_map_to_their_pages() {
        assert_eq!(link_url("claudeInstall"), Some("https://code.claude.com"));
        assert!(link_url("errorReporting")
            .is_some_and(|url| url.starts_with("https://github.com/ibrahimkobeissy/harneo/")));
    }

    #[test]
    fn unknown_names_and_raw_urls_are_refused() {
        assert_eq!(link_url(""), None);
        assert_eq!(link_url("https://example.com"), None);
        assert_eq!(link_url("ClaudeInstall"), None);
    }

    #[test]
    fn a_missing_launcher_falls_through_to_the_next() {
        let missing = Command::new("harneo-no-such-launcher");
        let present = Command::new(std::env::current_exe().unwrap_or_default());
        let mut present = present;
        present.arg("--list");
        assert!(spawn_first(vec![missing, present]).is_ok());
    }

    #[test]
    fn no_installed_launcher_is_an_error() {
        let err = spawn_first(vec![Command::new("harneo-no-such-launcher")]);
        assert_eq!(err.map_err(|e| e.kind()), Err(io::ErrorKind::NotFound));
        let none = spawn_first(Vec::new());
        assert_eq!(none.map_err(|e| e.kind()), Err(io::ErrorKind::NotFound));
    }

    #[test]
    fn a_launcher_that_cannot_start_is_reported() {
        // A directory exists but is not a program: the error is not "not found", so no fallback.
        let dir = Command::new(std::env::temp_dir());
        let err = spawn_first(vec![dir, Command::new("harneo-no-such-launcher")]);
        assert!(err.is_err_and(|e| e.kind() != io::ErrorKind::NotFound));
    }
}
