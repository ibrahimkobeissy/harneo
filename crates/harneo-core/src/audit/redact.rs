//! Replaces secret values in text that is about to be shown or stored (evidence, messages).
//! Rule tickets extend the pattern list; the rule is that a finding never carries a value.

use std::sync::LazyLock;

use regex::Regex;

use crate::artifact::literal_secret::REDACTED;

/// Known token shapes: a prefix and a body of at least 8 token characters.
const PATTERNS: [&str; 5] = [
    r"(?:gh[pousr]_|github_pat_|xox[abposr]-|sk-ant-|sk_live_|sk-|AKIA|AIza)[A-Za-z0-9_\-]{8,}",
    r"(?i)\bbearer\s+[A-Za-z0-9._~+/=\-]{8,}",
    r"eyJ[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}\.[A-Za-z0-9_\-]{5,}",
    r"://[^\s/@:]+:[^\s/@]+@",
    r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
];

static COMPILED: LazyLock<Vec<Regex>> =
    LazyLock::new(|| PATTERNS.iter().filter_map(|p| Regex::new(p).ok()).collect());

/// `text` with every secret-shaped value replaced by `<redacted>`.
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    for (i, re) in COMPILED.iter().enumerate() {
        let replacement = match i {
            1 => format!("Bearer {REDACTED}"),
            3 => format!("://{REDACTED}@"),
            _ => REDACTED.to_owned(),
        };
        out = re.replace_all(&out, replacement.as_str()).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        assert_eq!(COMPILED.len(), PATTERNS.len());
    }

    #[test]
    fn token_shapes_are_replaced() {
        let tail = "a1B2c3D4e5F6g7H8";
        for secret in [
            ["gh", "p_", tail].concat(),
            ["github_", "pat_", tail].concat(),
            ["sk", "-ant-", tail].concat(),
            ["AK", "IA", "ABCDEFGHIJKLMNOP"].concat(),
            ["xo", "xb-", tail].concat(),
        ] {
            let shown = redact(&format!("export X={secret} ok"));
            assert_eq!(shown, "export X=<redacted> ok", "{secret}");
        }
    }

    #[test]
    fn bearer_urls_jwts_and_pem_headers_are_replaced() {
        assert_eq!(
            redact("Authorization: Bearer abcdef123456"),
            "Authorization: Bearer <redacted>"
        );
        assert_eq!(
            redact("https://user:hunter22@host/x"),
            "https://<redacted>@host/x"
        );
        let jwt = ["ey", "Jhbgci", ".", "eyJzdWIi", ".", "abcdefgh"].concat();
        assert_eq!(redact(&jwt), "<redacted>");
        let pem = ["-----BEGIN ", "RSA PRIVATE", " KEY-----"].concat();
        assert_eq!(redact(&pem), "<redacted>");
    }

    #[test]
    fn plain_text_is_untouched() {
        let s = "run the sk-learn tests with https://example.com/a";
        assert_eq!(redact(s), s);
    }
}
