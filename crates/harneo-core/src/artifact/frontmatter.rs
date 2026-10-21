//! Splitting and parsing YAML frontmatter, shared by every Markdown artifact type.
//! Parsing runs with the YAML parser's size and alias limits on, so a crafted file cannot
//! exhaust memory, and a failure is a value to report, never a panic.

use serde_json::{Map, Value};

/// A Markdown file split into its metadata and its text.
#[derive(Debug, Clone, PartialEq)]
pub enum Frontmatter {
    /// The file has no frontmatter block.
    Absent { body: String },
    /// The block parsed into a mapping.
    Parsed {
        data: Map<String, Value>,
        body: String,
    },
    /// The block exists but is not valid YAML, not a mapping, or never closed.
    Invalid { message: String, body: String },
}

impl Frontmatter {
    /// The parsed fields; empty when the block is absent or invalid.
    pub fn data(&self) -> Map<String, Value> {
        match self {
            Self::Parsed { data, .. } => data.clone(),
            _ => Map::new(),
        }
    }
}

/// Splits `---` fenced frontmatter from the body. Returns `(yaml, body)`, `None` when the file
/// does not start with a fence, and `Err` when the fence is never closed.
fn split(contents: &str) -> Option<Result<(String, String), ()>> {
    let text = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let mut lines = text.split_inclusive('\n');
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut yaml = String::new();
    for line in lines.by_ref() {
        if matches!(line.trim_end(), "---" | "...") {
            let body: String = lines.collect();
            return Some(Ok((yaml, body)));
        }
        yaml.push_str(line);
    }
    Some(Err(()))
}

fn options() -> serde_saphyr::Options {
    let mut o = serde_saphyr::Options::default();
    let mut budget = serde_saphyr::Budget::default();
    budget.max_events = 100_000;
    budget.max_aliases = 1_000;
    budget.max_anchors = 1_000;
    budget.max_depth = 32;
    budget.max_nodes = 50_000;
    o.budget = Some(budget);
    o.alias_limits.max_total_replayed_events = 100_000;
    o.alias_limits.max_alias_expansions_per_anchor = 100;
    // The snippet would quote the file's own text back into the report.
    o.with_snippet = false;
    o
}

/// Splits and parses `contents`. CRLF line endings and a leading byte-order mark are accepted.
pub fn parse(contents: &str) -> Frontmatter {
    let text = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let (yaml, body) = match split(contents) {
        None => {
            return Frontmatter::Absent {
                body: text.to_owned(),
            };
        }
        Some(Err(())) => {
            return Frontmatter::Invalid {
                message: "the frontmatter block is never closed with `---`".into(),
                body: text.to_owned(),
            };
        }
        Some(Ok(parts)) => parts,
    };
    match serde_saphyr::from_str_with_options::<Value>(&yaml, options()) {
        Ok(Value::Object(data)) => Frontmatter::Parsed { data, body },
        Ok(Value::Null) => Frontmatter::Parsed {
            data: Map::new(),
            body,
        },
        Ok(_) => Frontmatter::Invalid {
            message: "the frontmatter must be a mapping of fields".into(),
            body,
        },
        Err(e) => Frontmatter::Invalid {
            message: format!("the frontmatter is not valid YAML: {e}"),
            body,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fields_and_body() {
        let f = parse("---\nname: a\ndescription: d\n---\nHello\n");
        let Frontmatter::Parsed { data, body } = f else {
            panic!("expected parsed");
        };
        assert_eq!(data["name"], "a");
        assert_eq!(body, "Hello\n");
    }

    #[test]
    fn accepts_crlf_and_bom() {
        let f = parse("\u{feff}---\r\nname: a\r\n---\r\nbody\r\n");
        assert_eq!(f.data()["name"], "a");
    }

    #[test]
    fn no_frontmatter_is_absent() {
        assert_eq!(
            parse("# Title\n"),
            Frontmatter::Absent {
                body: "# Title\n".into()
            }
        );
        assert!(matches!(parse(""), Frontmatter::Absent { .. }));
    }

    #[test]
    fn malformed_yaml_is_invalid_not_a_panic() {
        let f = parse("---\nname: [unclosed\n  : : :\n---\nbody");
        assert!(matches!(f, Frontmatter::Invalid { .. }));
        assert!(f.data().is_empty());
    }

    #[test]
    fn unclosed_block_is_invalid() {
        assert!(matches!(
            parse("---\nname: a\n"),
            Frontmatter::Invalid { .. }
        ));
    }

    #[test]
    fn non_mapping_is_invalid_and_empty_block_is_empty() {
        assert!(matches!(
            parse("---\n- a\n- b\n---\n"),
            Frontmatter::Invalid { .. }
        ));
        let f = parse("---\n---\nbody");
        assert!(matches!(f, Frontmatter::Parsed { .. }));
        assert!(f.data().is_empty());
    }

    #[test]
    fn alias_bomb_is_rejected_by_the_limits() {
        let mut y = String::from("---\na: &a [x, x, x, x, x, x, x, x, x, x]\n");
        let mut prev = 'a';
        for c in 'b'..='k' {
            let refs = format!("*{prev}, ").repeat(10);
            y.push_str(&format!("{c}: &{c} [{refs}]\n"));
            prev = c;
        }
        y.push_str("---\n");
        assert!(matches!(parse(&y), Frontmatter::Invalid { .. }));
    }
}
