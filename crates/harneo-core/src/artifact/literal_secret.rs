//! Shared check for literal secrets in `env` maps (Settings, MCP). A heuristic, not a scanner:
//! it only decides whether a value should have been a `${VAR}` placeholder. Callers report the
//! key name and must never print, log or store the value.

/// Words in a key name that suggest the value is a secret (compared upper-case).
const SECRET_WORDS: [&str; 8] = [
    "TOKEN",
    "KEY",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "AUTH",
    "BEARER",
];

/// What stands in for a value that was redacted before storing.
pub const REDACTED: &str = "<redacted>";

/// True when `value` is a literal secret rather than a placeholder: it is not a `${VAR}` or
/// `$VAR` reference, and either the key name suggests a secret or the value has a known token
/// format (`sk-`, `ghp_`, `github_pat_`, `xoxb-`/`xoxp-`, AWS access key ids).
pub fn is_literal_secret(key: &str, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || is_placeholder(value) {
        return false;
    }
    let key = key.to_ascii_uppercase();
    SECRET_WORDS.iter().any(|w| key.contains(w)) || has_token_format(value)
}

/// `${VAR}`, `${VAR:-default}` or `$VAR`.
fn is_placeholder(value: &str) -> bool {
    if let Some(inner) = value.strip_prefix("${") {
        return inner
            .strip_suffix('}')
            .is_some_and(|body| !body.contains('}'));
    }
    value.strip_prefix('$').is_some_and(|name| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn has_token_format(value: &str) -> bool {
    let tail_ok = |tail: &str, min: usize| {
        tail.len() >= min
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    const PREFIXES: [&str; 13] = [
        "sk-",
        "ghp_",
        "github_pat_",
        "xoxb-",
        "xoxp-",
        "gho_",
        "ghs_",
        "ghu_",
        "ghr_",
        "xoxa-",
        "xoxs-",
        "sk_live_",
        "rk_live_",
    ];
    if PREFIXES
        .iter()
        .any(|p| value.strip_prefix(p).is_some_and(|t| tail_ok(t, 8)))
    {
        return true;
    }
    if value
        .strip_prefix("AIza")
        .is_some_and(|t| t.len() == 35 && tail_ok(t, 35))
    {
        return true;
    }
    value.strip_prefix("AKIA").is_some_and(|t| {
        t.len() == 16
            && t.chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_looking_key_with_a_literal_value_triggers() {
        for key in [
            "API_TOKEN",
            "db_password",
            "MY_SECRET",
            "STRIPE_KEY",
            "AUTH",
        ] {
            assert!(
                is_literal_secret(key, "harneo-test-literal-value-123"),
                "{key}"
            );
        }
    }

    #[test]
    fn placeholders_empty_values_and_plain_keys_do_not() {
        assert!(!is_literal_secret("API_TOKEN", "${API_TOKEN}"));
        assert!(!is_literal_secret("API_TOKEN", "${API_TOKEN:-none}"));
        assert!(!is_literal_secret("API_TOKEN", "$API_TOKEN"));
        assert!(!is_literal_secret("API_TOKEN", ""));
        assert!(!is_literal_secret("LOG_LEVEL", "debug"));
        assert!(!is_literal_secret("NODE_ENV", "production"));
    }

    #[test]
    fn a_placeholder_with_trailing_text_is_still_literal() {
        assert!(is_literal_secret("API_TOKEN", "${PREFIX}literal-part"));
    }

    #[test]
    fn token_formats_trigger_under_any_key() {
        // Built at run time so no token-shaped string sits in the source.
        let fakes = [
            format!("{}-{}", "sk", "fakefake1"),
            format!("{}_{}", "ghp", "fakefake1"),
            format!("{}_{}", "github_pat", "fakefake1"),
            format!("{}-{}", "xoxb", "fakefake1"),
            format!("{}{}", "AKIA", "0".repeat(16)),
        ];
        for v in &fakes {
            assert!(is_literal_secret("SOMETHING", v), "{v}");
        }
    }

    #[test]
    fn extended_token_formats_trigger() {
        let fakes = [
            format!("{}_{}", "gho", "fakefake1"),
            format!("{}_{}", "ghs", "fakefake1"),
            format!("{}_{}", "sk_live", "fakefake1"),
            format!("{}{}", "AIza", "B".repeat(35)),
        ];
        for v in &fakes {
            assert!(is_literal_secret("SOMETHING", v), "{v}");
        }
        assert!(!is_literal_secret(
            "SOMETHING",
            &format!("AIza{}", "B".repeat(10))
        ));
    }

    #[test]
    fn near_misses_of_token_formats_do_not() {
        assert!(!is_literal_secret("SOMETHING", "sk-"));
        assert!(!is_literal_secret("SOMETHING", "sk-ab"));
        assert!(!is_literal_secret(
            "SOMETHING",
            &format!("AKIA{}", "0".repeat(15))
        ));
        assert!(!is_literal_secret("SOMETHING", "ask-fakefake1"));
    }
}
