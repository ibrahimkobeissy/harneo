//! Optional error reporting to Sentry, shared by the desktop app and the CLI.
//!
//! Reporting is off unless both hold: the `HARNEO_SENTRY_DSN` environment variable is set and the
//! Settings switch (`diagnostics.sentryReports`) is on. The DSN is read at runtime, never
//! committed, never baked in, never logged. Only errors are sent: no tracing, no sessions, no
//! default PII. Every event passes `scrub_event` first; `harneo-core` stays free of reporting.

use std::borrow::Cow;
use std::sync::{Arc, LazyLock};

use regex::Regex;
use sentry::protocol::{Breadcrumb, Event, Frame, Stacktrace};
use sentry::ClientOptions;

/// The app setting behind Settings → Diagnostics.
pub const SETTING_KEY: &str = "diagnostics.sentryReports";
/// Messages are cut to this many characters.
pub const MAX_MESSAGE: usize = 500;

/// Which part of Harneo reports; sent as the `side` tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The Rust backend of the desktop app.
    RustApp,
    /// The `harneo` command.
    Cli,
}

impl Side {
    /// The tag value.
    pub fn tag(self) -> &'static str {
        match self {
            Side::RustApp => "rust-app",
            Side::Cli => "cli",
        }
    }
}

/// The environment variable that holds the DSN.
pub const DSN_ENV: &str = "HARNEO_SENTRY_DSN";

/// The DSN in `HARNEO_SENTRY_DSN`, or `None` when it is unset or blank: there is no default.
pub fn dsn_from_env() -> Option<String> {
    dsn_from(std::env::var(DSN_ENV).ok())
}

fn dsn_from(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// The DSN to report with, or `None` when reporting is off: both a DSN and the switch are needed.
pub fn decide(dsn: Option<String>, enabled: bool) -> Option<String> {
    if enabled {
        dsn.filter(|d| !d.trim().is_empty())
    } else {
        None
    }
}

/// Whether the Settings switch is on in the library; off when unset or unreadable.
pub fn switch_on(library: &harneo_core::library::Library) -> bool {
    matches!(
        library.settings_get(SETTING_KEY),
        Ok(Some(serde_json::Value::Bool(true)))
    )
}

/// Reports an error text (scrubbed on the way out); a no-op when no client is running.
pub fn capture(text: &str) {
    sentry::capture_message(text, sentry::Level::Error);
}

/// Starts Sentry when `decide` says so; the guard flushes on drop. `None` creates no client.
pub fn init(side: Side, dsn: Option<String>, enabled: bool) -> Option<sentry::ClientInitGuard> {
    let dsn = decide(dsn, enabled)?;
    Some(sentry::init(build_options(&dsn, side)?))
}

/// Client options: errors only, scrubbed, tagged with the side and the release.
pub fn build_options(dsn: &str, side: Side) -> Option<ClientOptions> {
    let home = dirs::home_dir().map(|h| h.to_string_lossy().into_owned());
    let event_home = home.clone();
    let guard = QuotaGuard::default();
    let before_send: Arc<dyn Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync> =
        Arc::new(move |mut event| {
            event
                .tags
                .insert("side".to_string(), side.tag().to_string());
            let event = scrub_event(event, event_home.as_deref())?;
            guard.allow(&event).then_some(event)
        });
    let before_breadcrumb: Arc<dyn Fn(Breadcrumb) -> Option<Breadcrumb> + Send + Sync> =
        Arc::new(move |crumb| Some(scrub_breadcrumb(crumb, home.as_deref())));
    let mut options = ClientOptions::default();
    options.dsn = Some(dsn.parse().ok()?);
    options.release = Some(Cow::Owned(format!("harneo@{}", harneo_core::version())));
    options.environment = Some(Cow::Borrowed(if cfg!(debug_assertions) {
        "development"
    } else {
        "release"
    }));
    // Errors only: no default PII, and no tracing (the default strategy samples nothing).
    options.send_default_pii = false;
    options.server_name = None;
    options.before_send = Some(before_send);
    options.before_breadcrumb = Some(before_breadcrumb);
    Some(options)
}

/// Most events one process run sends (the free Sentry plan has a small monthly quota).
pub const MAX_EVENTS_PER_RUN: usize = 20;

/// Client-side quota: at most `MAX_EVENTS_PER_RUN` events per run, and an identical error (same
/// type and message, after scrubbing) once per run.
#[derive(Clone, Default)]
pub struct QuotaGuard {
    seen: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl QuotaGuard {
    /// Whether this (already scrubbed) event may be sent; counts it when it may.
    pub fn allow(&self, event: &Event<'static>) -> bool {
        let kind = event
            .exception
            .values
            .first()
            .map(|e| format!("{}:{}", e.ty, e.value.as_deref().unwrap_or("")))
            .unwrap_or_else(|| format!("msg:{}", event.message.as_deref().unwrap_or("")));
        let Ok(mut seen) = self.seen.lock() else {
            return false;
        };
        if seen.len() >= MAX_EVENTS_PER_RUN {
            return false;
        }
        seen.insert(kind)
    }
}

static USER_DIR: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?:/home|/Users)/[^/\\\s"']+|[A-Za-z]:\\Users\\[^\\\s"']+"#).ok()
});

/// Replaces the home folder with `~`, then cuts the text to `MAX_MESSAGE` characters.
pub fn scrub_text(text: &str, home: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(home) = home.filter(|h| h.len() > 1) {
        out = out.replace(home.trim_end_matches(['/', '\\']), "~");
    }
    if let Some(re) = USER_DIR.as_ref() {
        out = re.replace_all(&out, "~").into_owned();
    }
    if out.chars().count() > MAX_MESSAGE {
        out = out.chars().take(MAX_MESSAGE).collect();
    }
    out
}

fn scrub_opt(text: &mut Option<String>, home: Option<&str>) {
    if let Some(t) = text {
        *t = scrub_text(t, home);
    }
}

fn scrub_frame(frame: &mut Frame, home: Option<&str>) {
    scrub_opt(&mut frame.filename, home);
    scrub_opt(&mut frame.abs_path, home);
    scrub_opt(&mut frame.module, home);
    // Source lines and variables are file contents: never attached.
    frame.pre_context.clear();
    frame.context_line = None;
    frame.post_context.clear();
    frame.vars.clear();
}

fn scrub_stacktrace(trace: &mut Stacktrace, home: Option<&str>) {
    trace.frames.iter_mut().for_each(|f| scrub_frame(f, home));
}

/// A breadcrumb without its data payload, its text scrubbed.
pub fn scrub_breadcrumb(mut crumb: Breadcrumb, home: Option<&str>) -> Breadcrumb {
    crumb.data.clear();
    scrub_opt(&mut crumb.message, home);
    crumb
}

/// The event as it may leave the machine, or `None` when its text holds a token shape.
pub fn scrub_event(mut event: Event<'static>, home: Option<&str>) -> Option<Event<'static>> {
    if has_token(&event) {
        return None;
    }
    scrub_opt(&mut event.message, home);
    scrub_opt(&mut event.culprit, home);
    scrub_opt(&mut event.transaction, home);
    if let Some(entry) = event.logentry.as_mut() {
        entry.message = scrub_text(&entry.message, home);
        entry.params.clear();
    }
    for exception in event.exception.values.iter_mut() {
        scrub_opt(&mut exception.value, home);
        scrub_opt(&mut exception.module, home);
        if let Some(trace) = exception.stacktrace.as_mut() {
            scrub_stacktrace(trace, home);
        }
    }
    if let Some(trace) = event.stacktrace.as_mut() {
        scrub_stacktrace(trace, home);
    }
    for thread in event.threads.values.iter_mut() {
        if let Some(trace) = thread.stacktrace.as_mut() {
            scrub_stacktrace(trace, home);
        }
    }
    event.breadcrumbs.values = std::mem::take(&mut event.breadcrumbs.values)
        .into_iter()
        .map(|c| scrub_breadcrumb(c, home))
        .collect();
    event.request = None;
    event.extra.clear();
    event.user = None;
    event.server_name = None;
    (!has_token(&event)).then_some(event)
}

fn has_token(event: &Event<'static>) -> bool {
    serde_json::to_string(event).is_ok_and(|json| harneo_core::audit::contains_token_shape(&json))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Exception, Request};
    use sentry::test::with_captured_events_options;

    #[test]
    fn the_dsn_comes_only_from_the_environment_with_no_default() {
        assert_eq!(dsn_from(None), None);
        assert_eq!(dsn_from(Some(String::new())), None);
        assert_eq!(dsn_from(Some("  ".into())), None);
        assert_eq!(
            dsn_from(Some(" https://x@y/1 ".into())).as_deref(),
            Some("https://x@y/1")
        );
    }

    const FAKE_DSN: &str = "https://public@example.invalid/1";

    fn token() -> String {
        format!("ghp_{}", "a1B2".repeat(10))
    }

    #[test]
    fn needs_both_dsn_and_switch() {
        let dsn = Some("https://k@example.invalid/1".to_string());
        assert!(decide(dsn.clone(), false).is_none());
        assert!(decide(None, true).is_none());
        assert!(decide(Some("  ".into()), true).is_none());
        assert_eq!(decide(dsn.clone(), true), dsn);
    }

    #[test]
    fn no_client_without_dsn_or_switch() {
        assert!(init(Side::Cli, Some(FAKE_DSN.into()), false).is_none());
        assert!(init(Side::Cli, None, true).is_none());
        assert!(sentry::Hub::current().client().is_none());
    }

    #[test]
    fn a_bad_dsn_creates_no_client() {
        assert!(init(Side::Cli, Some("not a dsn".into()), true).is_none());
    }

    #[test]
    fn sends_a_tagged_release_event_through_the_mock_transport() {
        for side in [Side::RustApp, Side::Cli] {
            let options = build_options(FAKE_DSN, side).expect("options");
            let events = with_captured_events_options(
                || {
                    sentry::capture_message("boom", sentry::Level::Error);
                },
                options,
            );
            assert_eq!(events.len(), 1);
            let event = &events[0];
            assert_eq!(event.tags.get("side").map(String::as_str), Some(side.tag()));
            assert_eq!(
                event.release.as_deref(),
                Some(format!("harneo@{}", harneo_core::version()).as_str())
            );
            assert!(event.environment.is_some());
        }
    }

    #[test]
    fn quota_guard_caps_events_and_dedupes() {
        let guard = QuotaGuard::default();
        let msg = |m: &str| Event {
            message: Some(m.into()),
            ..Event::default()
        };
        assert!(guard.allow(&msg("same")));
        assert!(!guard.allow(&msg("same")));
        for i in 1..MAX_EVENTS_PER_RUN {
            assert!(guard.allow(&msg(&format!("e{i}"))));
        }
        assert!(!guard.allow(&msg("one more")));
    }

    #[test]
    fn identical_errors_are_sent_once_through_the_client() {
        let options = build_options(FAKE_DSN, Side::Cli).expect("options");
        let events = with_captured_events_options(
            || {
                sentry::capture_message("again /home/ana/x", sentry::Level::Error);
                sentry::capture_message("again /home/bo/x", sentry::Level::Error);
            },
            options,
        );
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn home_becomes_tilde() {
        assert_eq!(scrub_text("/home/ana/x", None), "~/x");
        assert_eq!(scrub_text("at /Users/bo/y z", None), "at ~/y z");
        assert_eq!(scrub_text(r"C:\Users\cy\z", None), r"~\z");
        assert_eq!(scrub_text("/data/me/f", Some("/data/me")), "~/f");
        assert_eq!(scrub_text("/data/me/f", Some("/")), "/data/me/f");
    }

    #[test]
    fn messages_are_truncated() {
        assert_eq!(scrub_text(&"é".repeat(900), None).chars().count(), 500);
    }

    #[test]
    fn breadcrumb_data_is_dropped() {
        let mut crumb = Breadcrumb {
            message: Some("open /home/ana/a".into()),
            ..Breadcrumb::default()
        };
        crumb.data.insert("body".into(), "secret text".into());
        let crumb = scrub_breadcrumb(crumb, None);
        assert!(crumb.data.is_empty());
        assert_eq!(crumb.message.as_deref(), Some("open ~/a"));
    }

    #[test]
    fn event_is_scrubbed_everywhere() {
        let mut event = Event {
            message: Some("failed /home/ana/x".into()),
            ..Event::default()
        };
        event.extra.insert("file".into(), "contents".into());
        event.request = Some(Request {
            data: Some("body".into()),
            ..Request::default()
        });
        let frame = Frame {
            abs_path: Some("/home/ana/src/a.rs".into()),
            context_line: Some("let x = 1;".into()),
            pre_context: vec!["a".into()],
            ..Frame::default()
        };
        event.exception.values.push(Exception {
            value: Some("bad /home/ana/q".into()),
            stacktrace: Some(Stacktrace {
                frames: vec![frame],
                ..Stacktrace::default()
            }),
            ..Exception::default()
        });
        let mut crumb = Breadcrumb::default();
        crumb.data.insert("k".into(), "v".into());
        event.breadcrumbs.values.push(crumb);
        let out = scrub_event(event, None).expect("kept");
        assert_eq!(out.message.as_deref(), Some("failed ~/x"));
        assert!(out.extra.is_empty() && out.request.is_none());
        let ex = &out.exception.values[0];
        assert_eq!(ex.value.as_deref(), Some("bad ~/q"));
        let f = &ex.stacktrace.as_ref().expect("trace").frames[0];
        assert_eq!(f.abs_path.as_deref(), Some("~/src/a.rs"));
        assert!(f.context_line.is_none() && f.pre_context.is_empty());
        assert!(out.breadcrumbs.values[0].data.is_empty());
    }

    #[test]
    fn an_event_with_a_token_shape_is_dropped() {
        let event = Event {
            message: Some(format!("auth failed with {}", token())),
            ..Event::default()
        };
        assert!(scrub_event(event, None).is_none());
        let options = build_options(FAKE_DSN, Side::Cli).expect("options");
        let events = with_captured_events_options(
            || {
                sentry::capture_message(&format!("x {}", token()), sentry::Level::Error);
            },
            options,
        );
        assert!(events.is_empty());
    }
}
