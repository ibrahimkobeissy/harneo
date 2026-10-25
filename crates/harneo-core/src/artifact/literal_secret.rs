//! Shared check for literal secrets in `env` maps (Settings, MCP). A heuristic, not a scanner:
//! it only decides whether a value should have been a `${VAR}` placeholder. Callers report the
//! key name and must never print, log or store the value.

/// Whole words in a key name that suggest the value is a secret (compared lower-case).
const SECRET_WORDS: [&str; 12] = [
    "token",
    "tokens",
    "secret",
    "secrets",
    "password",
    "passwd",
    "credential",
    "credentials",
    "auth",
    "authorization",
    "bearer",
    "apikey",
];

/// Words that make a value a likely secret even when it is a plain lower-case phrase
/// (`DB_PASSWORD=correct-horse-battery`); the others also need mixed character classes.
const STRONG_WORDS: [&str; 6] = [
    "password",
    "passwd",
    "secret",
    "secrets",
    "credential",
    "credentials",
];

/// Words that only count in a compound name (`apiKey`, `secret_key`), never alone.
const COMPOUND_WORDS: [&str; 1] = ["key"];

/// Splits a key into lower-case words: camelCase, snake_case, kebab-case, dots, `HTTPServer`.
fn key_words(key: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = key.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let boundary = c.is_ascii_uppercase()
            && prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || p.is_ascii_digit()
                    || (p.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()))
            });
        if boundary && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(c.to_ascii_lowercase());
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// `None` when the key name is not secret-sounding, else whether it is a strong word.
fn secret_key_strength(key: &str) -> Option<bool> {
    let words = key_words(key);
    let strong = words.iter().any(|w| STRONG_WORDS.contains(&w.as_str()));
    let weak = words.iter().any(|w| SECRET_WORDS.contains(&w.as_str()))
        || (words.len() >= 2 && words.iter().any(|w| COMPOUND_WORDS.contains(&w.as_str())));
    (strong || weak).then_some(strong)
}

/// Whether `value` could be a credential when only the key name suggested a secret: no
/// whitespace, 12+ characters, not an enum constant, slug, path or plain URL, mixed classes.
fn looks_like_credential(value: &str, strong_key: bool) -> bool {
    if value.len() < 12 || value.contains(char::is_whitespace) {
        return false;
    }
    let has_letter = value.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    let upper_snake = !has_digit && value.chars().all(|c| c.is_ascii_uppercase() || c == '_');
    let slug = value
        .chars()
        .all(|c| c.is_ascii_lowercase() || matches!(c, '-' | '_' | '.'));
    if upper_snake || (slug && !strong_key) {
        return false;
    }
    if value.starts_with(['/', '~']) || value.starts_with("./") || value.starts_with("../") {
        return false;
    }
    if value.contains("://") && !value.contains('@') {
        return false;
    }
    let mixed_case = value.chars().any(|c| c.is_ascii_uppercase())
        && value.chars().any(|c| c.is_ascii_lowercase());
    strong_key || (has_letter && has_digit) || (mixed_case && entropy(value) >= 3.5)
}

/// Shannon entropy in bits per character.
fn entropy(value: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in value.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let n = value.chars().count() as f64;
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum()
}

/// What stands in for a value that was redacted before storing.
pub const REDACTED: &str = "<redacted>";

/// True when `value` is a literal secret rather than a placeholder: it is not a `${VAR}` or
/// `$VAR` reference, and either the key name suggests a secret (whole words) and the value looks
/// like a credential, or the value has a known token format (`sk-`, `ghp_`, `github_pat_`, `xoxb-`/`xoxp-`, AWS access key ids).
pub fn is_literal_secret(key: &str, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let strength = secret_key_strength(key);
    let secret_key = strength.is_some();
    if is_placeholder(value) {
        // A `${VAR:-default}` placeholder is only as safe as its default.
        return default_part(value).is_some_and(|d| {
            has_token_format(d) || (secret_key && d.len() >= 16 && !d.contains(char::is_whitespace))
        });
    }
    strength.is_some_and(|strong| looks_like_credential(value, strong)) || has_token_format(value)
}

/// The default of a `${VAR:-default}` / `${VAR:=default}` / `${VAR-default}` reference.
fn default_part(value: &str) -> Option<&str> {
    let body = value.strip_prefix("${")?.strip_suffix('}')?;
    let at = body.find(['-', '='])?;
    let rest = &body[at + 1..];
    let name = body[..at].trim_end_matches(':');
    (!name.is_empty() && !rest.is_empty()).then_some(rest)
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
    fn key_names_match_whole_words_only() {
        let v = "Zk3mQ9xT7pLw2Vb8Nc4Rd5Hs";
        for key in [
            "apiKey",
            "API_KEY",
            "secretKey",
            "privateKey",
            "accessKey",
            "auth_token",
            "password",
            "client-secret",
            "db.password",
        ] {
            assert!(is_literal_secret(key, v), "{key}");
        }
        for key in [
            "key",
            "id",
            "AUTHOR",
            "GIT_AUTHOR_NAME",
            "authentication",
            "monkey",
        ] {
            assert!(!is_literal_secret(key, v), "{key}");
        }
    }

    #[test]
    fn a_key_name_alone_needs_a_credential_looking_value() {
        assert!(is_literal_secret("password", "hunter2hunter2X9"));
        assert!(!is_literal_secret("auth", "session-radar"));
        for v in [
            "ON_INSTALL",
            "glob*chars? [and] \\backslashes",
            "Floor Guard Test",
            "short1",
            "/usr/local/share/app",
            "https://example.com/api",
        ] {
            assert!(!is_literal_secret("secret", v), "{v}");
        }
    }

    #[test]
    fn token_formats_trigger_under_the_bare_key_name() {
        assert!(is_literal_secret(
            "key",
            &format!("{}_{}", "ghp", "fakefake1")
        ));
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
    fn a_token_hidden_in_a_default_is_literal_but_harmless_defaults_are_not() {
        let token = ["gh", "p_", &"a".repeat(36)].concat();
        assert!(is_literal_secret(
            "API_KEY",
            &format!("${{API_KEY:-{token}}}")
        ));
        assert!(is_literal_secret("OTHER", &format!("${{OTHER:={token}}}")));
        assert!(is_literal_secret(
            "API_KEY",
            &format!("${{API_KEY-{token}}}")
        ));
        assert!(is_literal_secret(
            "API_KEY",
            "${API_KEY:-abcdefghijklmnopqrstuvwx}"
        ));
        assert!(!is_literal_secret("PORT", "${PORT:-8080}"));
        assert!(!is_literal_secret("API_KEY", "${API_KEY:-}"));
        assert!(!is_literal_secret("API_KEY", "${API_KEY}"));
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
