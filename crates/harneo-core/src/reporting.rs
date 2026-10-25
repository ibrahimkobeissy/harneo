//! Where error reports go. Core holds only the address; the Sentry SDKs live in the app and the CLI.

/// Harneo's Sentry client key. It is write-only (it can send events, not read any), which is
/// what Sentry designs DSNs to be, so it is safe to publish in the source.
pub const DEFAULT_SENTRY_DSN: &str = "https://a2c9af7cfd9a27a2a33349ab647dc716@o4512075679465472.ingest.de.sentry.io/4512316008955984";

/// The environment variable that overrides the DSN; set empty, it disables reporting.
pub const DSN_ENV: &str = "HARNEO_SENTRY_DSN";

fn resolve(env: Option<String>) -> Option<String> {
    match env {
        Some(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        None => Some(DEFAULT_SENTRY_DSN.to_string()),
    }
}

/// The DSN to report with: `HARNEO_SENTRY_DSN` when set (empty means disabled), else the default.
pub fn sentry_dsn() -> Option<String> {
    resolve(std::env::var(DSN_ENV).ok())
}

/// Whether `HARNEO_SENTRY_DSN` is set to an empty value, which disables reporting.
pub fn reporting_disabled_by_env() -> bool {
    sentry_dsn().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_wrappers_agree() {
        assert_eq!(reporting_disabled_by_env(), sentry_dsn().is_none());
    }

    #[test]
    fn unset_uses_the_default_set_overrides_empty_disables() {
        assert_eq!(resolve(None).as_deref(), Some(DEFAULT_SENTRY_DSN));
        assert_eq!(
            resolve(Some(" https://x@y/1 ".into())).as_deref(),
            Some("https://x@y/1")
        );
        assert_eq!(resolve(Some(String::new())), None);
        assert_eq!(resolve(Some("  ".into())), None);
    }
}
