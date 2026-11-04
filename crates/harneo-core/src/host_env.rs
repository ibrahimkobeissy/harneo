//! The environment for host programs (git, claude) started from a packaged Harneo.
//!
//! An AppImage's launcher points `LD_LIBRARY_PATH`, `PATH`, `XDG_DATA_DIRS` and a few GTK
//! variables into its mount (`$APPDIR`) so the app finds its bundled libraries. A host program
//! started with that environment loads the bundle's libraries instead of its own and can crash
//! (system libcurl with the bundle's older libnghttp2, for instance). Host programs get the
//! environment without the bundle: every `$APPDIR` entry is dropped. Outside an AppImage
//! (`APPDIR` unset: source builds, the .deb) nothing changes.

use std::ffi::OsString;
use std::process::Command;

/// Removes the AppImage bundle from `command`'s environment, when running from an AppImage.
pub fn for_host(command: &mut Command) {
    let Some(appdir) = std::env::var("APPDIR")
        .ok()
        .filter(|dir| dir.starts_with('/'))
    else {
        return;
    };
    for (name, value) in changes(&appdir, std::env::vars_os()) {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
}

/// The variables to change so none points into `appdir`: `Some` is the new value, `None`
/// removes the variable. Path lists keep their other entries in order.
fn changes(
    appdir: &str,
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, Option<OsString>)> {
    let appdir = appdir.trim_end_matches('/');
    let inside = |entry: &str| {
        entry == appdir
            || entry
                .strip_prefix(appdir)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    let mut out = Vec::new();
    for (name, value) in vars {
        // The AppImage's own markers name the bundle on purpose; they don't redirect anything.
        if matches!(name.to_str(), Some("APPDIR" | "APPIMAGE" | "ARGV0" | "OWD")) {
            continue;
        }
        let Some(value) = value.to_str() else {
            continue;
        };
        let entries: Vec<&str> = value.split(':').collect();
        if !entries.iter().any(|entry| inside(entry)) {
            continue;
        }
        let kept: Vec<&str> = entries
            .into_iter()
            .filter(|entry| !entry.is_empty() && !inside(entry))
            .collect();
        let new = (!kept.is_empty()).then(|| OsString::from(kept.join(":")));
        out.push((name, new));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    fn change(name: &str, value: Option<&str>) -> (OsString, Option<OsString>) {
        (OsString::from(name), value.map(OsString::from))
    }

    #[test]
    fn bundle_entries_leave_path_lists_in_order() {
        let got = changes(
            "/tmp/.mount_HarneoX",
            vars(&[
                ("LD_LIBRARY_PATH", "/tmp/.mount_HarneoX/usr/lib:/opt/lib"),
                (
                    "PATH",
                    "/tmp/.mount_HarneoX/usr/bin:/usr/local/bin:/usr/bin",
                ),
            ]),
        );
        assert_eq!(
            got,
            vec![
                change("LD_LIBRARY_PATH", Some("/opt/lib")),
                change("PATH", Some("/usr/local/bin:/usr/bin")),
            ]
        );
    }

    #[test]
    fn a_variable_left_empty_is_removed() {
        let got = changes(
            "/tmp/.mount_HarneoX/",
            vars(&[
                ("LD_LIBRARY_PATH", "/tmp/.mount_HarneoX/usr/lib:"),
                (
                    "GDK_PIXBUF_MODULE_FILE",
                    "/tmp/.mount_HarneoX/usr/lib/loaders.cache",
                ),
                ("GTK_PATH", "/tmp/.mount_HarneoX"),
            ]),
        );
        assert_eq!(
            got,
            vec![
                change("LD_LIBRARY_PATH", None),
                change("GDK_PIXBUF_MODULE_FILE", None),
                change("GTK_PATH", None),
            ]
        );
    }

    #[test]
    fn unrelated_and_lookalike_paths_and_markers_are_kept() {
        let got = changes(
            "/tmp/.mount_HarneoX",
            vars(&[
                ("HOME", "/home/me"),
                ("XDG_DATA_DIRS", "/tmp/.mount_HarneoXY/share:/usr/share"),
                ("APPDIR", "/tmp/.mount_HarneoX"),
                ("APPIMAGE", "/home/me/Harneo.AppImage"),
                ("OWD", "/tmp/.mount_HarneoX"),
            ]),
        );
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn outside_an_appimage_the_command_is_untouched() {
        // The test runner is not an AppImage, so APPDIR is unset and no variable is set.
        if std::env::var_os("APPDIR").is_some() {
            return;
        }
        let mut command = Command::new("git");
        for_host(&mut command);
        assert_eq!(command.get_envs().count(), 0);
    }
}
