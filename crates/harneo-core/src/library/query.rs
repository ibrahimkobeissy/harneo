//! Listing the library: filters, sort, pages and facet counts.
//!
//! One [`Filter`] (types, tags, sources, text) drives both the rows and the facets, and is built
//! to be reused: smart collections store a `Filter` as their rule and evaluate it with
//! [`filter_clause`].
//!
//! Semantics: values inside one filter are OR-ed, different filters are AND-ed. Text is a
//! case-insensitive substring match (ASCII case folding, as SQLite's `LIKE` does; `%`, `_` and
//! `\` in the text are literal) against the name, the description, the source label and the tag
//! names. The source label is the folder's basename for a local source; for any other source it
//! is the whole location (URL), which contains `owner/repo`.
//!
//! Facets are standard faceted search: each facet's counts are taken over the artifacts that
//! match every filter *except that facet's own*, so picking a tag does not make the other tags
//! vanish. Kinds list only values present in that set (plus selected ones, at 0); tags and
//! sources list every known value, with 0 where the rest of the filter excludes it.

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::artifact::ArtifactKind;
use crate::audit::Tier;
use crate::error::Result;

/// Most rows one page may hold; a larger `limit` is clamped to this.
pub const MAX_LIMIT: u32 = 1000;

/// SQL for a source's label (alias `s` is the source row): the basename of a local folder, the
/// location itself otherwise.
const LABEL_SQL: &str = "(CASE WHEN s.kind = 'local' \
    THEN replace(s.location, rtrim(s.location, replace(s.location, '/', '')), '') \
    ELSE s.location END)";

/// What to keep. Empty lists and empty text mean no restriction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, rename_all = "camelCase")]
pub struct Filter {
    pub types: Vec<ArtifactKind>,
    /// Tag names (case-insensitive).
    pub tags: Vec<String>,
    /// Source ids.
    pub sources: Vec<String>,
    pub text: Option<String>,
    /// Keep artifacts scored at least this (0 to 100); unaudited ones never match.
    pub trust_min: Option<u8>,
    /// Trust tiers; unaudited artifacts never match.
    pub tiers: Vec<Tier>,
    /// Only artifacts whose audit has at least one finding.
    pub has_findings: bool,
}

/// What to sort the list by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SortKey {
    /// Case-insensitive.
    #[default]
    Name,
    Kind,
    /// By source label.
    Source,
    TagCount,
    ImportedAt,
    /// By trust score; artifacts not audited yet come last in both directions.
    Trust,
    /// A manual collection's own order. Only meaningful in `Library::collection_page` of a
    /// manual collection; everywhere else it sorts like `Name`.
    Position,
}

/// One list request. The default is the first 50 artifacts by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, rename_all = "camelCase")]
pub struct Query {
    pub filter: Filter,
    pub sort: SortKey,
    pub descending: bool,
    pub offset: u32,
    /// Clamped to [`MAX_LIMIT`].
    pub limit: u32,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            sort: SortKey::default(),
            descending: false,
            offset: 0,
            limit: 50,
        }
    }
}

/// Where a listed artifact came from, in short.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    /// `local` or `git`.
    pub kind: String,
    /// The folder's basename (local) or `owner/repo` (git).
    pub label: String,
    pub branch: Option<String>,
}

/// A tag on a listed artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TagChip {
    pub name: String,
    pub color: String,
}

/// What a card or row shows; the full artifact is `Library::detail`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub kind: ArtifactKind,
    pub name: String,
    pub description: Option<String>,
    pub source: SourceSummary,
    /// Sorted by name.
    pub tags: Vec<TagChip>,
    /// Unix seconds.
    pub imported_at: i64,
    pub error_count: u32,
    pub warning_count: u32,
    /// Files that travel with the artifact.
    pub file_count: u32,
    /// References the artifact makes to other artifacts.
    pub reference_count: u32,
    /// How many of those references were found in the same scan.
    pub resolved_references: u32,
    /// The stored trust score; `None` until the artifact is audited.
    pub trust_score: Option<u8>,
    pub tier: Option<Tier>,
    /// Findings in the stored audit (0 when not audited).
    pub finding_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct KindCount {
    pub kind: ArtifactKind,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TierCount {
    pub tier: Tier,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TagCount {
    pub name: String,
    pub color: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SourceCount {
    pub source: SourceSummary,
    pub count: u32,
}

/// Counts per filter value (see the module docs for what they are counted over).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Facets {
    /// In `ArtifactKind` declaration order.
    pub kinds: Vec<KindCount>,
    /// Sorted by name.
    pub tags: Vec<TagCount>,
    /// Sorted by label, then id.
    pub sources: Vec<SourceCount>,
    /// In `Tier` order (best first); only tiers present in that set, plus selected ones at 0.
    pub tiers: Vec<TierCount>,
}

/// One page of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct Page {
    pub items: Vec<LibraryItem>,
    /// Artifacts matching the filter, across all pages.
    pub total: u32,
    /// Artifacts in the library (or the collection) before any filter: the "M" of "N of M".
    pub unfiltered_total: u32,
    pub facets: Facets,
}

/// An extra restriction ANDed to the query's own filter (and to every facet), with the manual
/// collection whose order `SortKey::Position` follows, if any.
#[derive(Debug, Clone, Default)]
pub(crate) struct Scope {
    /// A condition over `artifacts a JOIN sources s`, with `?` placeholders for `params`.
    pub clause: String,
    pub params: Vec<Value>,
    pub manual_collection: Option<i64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Facet {
    Types,
    Tags,
    Sources,
    Tiers,
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

fn kind_name(kind: ArtifactKind) -> Result<String> {
    Ok(serde_json::to_value(kind)?
        .as_str()
        .unwrap_or_default()
        .to_owned())
}

fn tier_name(tier: Tier) -> Result<String> {
    super::records::tier_name(tier)
}

fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('%');
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// The `WHERE` condition and its bound values for `filter`, over `artifacts a JOIN sources s`.
/// Always a valid condition (`1` when the filter is empty). Values are only ever bound, never
/// spliced in. Smart collections evaluate their rule with this.
pub(crate) fn filter_clause(filter: &Filter) -> Result<(String, Vec<Value>)> {
    build(filter, None, None)
}

fn build(
    filter: &Filter,
    omit: Option<Facet>,
    scope: Option<&Scope>,
) -> Result<(String, Vec<Value>)> {
    let mut parts: Vec<String> = Vec::new();
    let mut params: Vec<Value> = Vec::new();
    if omit != Some(Facet::Types) && !filter.types.is_empty() {
        parts.push(format!("a.kind IN ({})", placeholders(filter.types.len())));
        for kind in &filter.types {
            params.push(Value::Text(kind_name(*kind)?));
        }
    }
    if omit != Some(Facet::Tags) && !filter.tags.is_empty() {
        parts.push(format!(
            "EXISTS (SELECT 1 FROM artifact_tags x JOIN tags t ON t.id = x.tag_id \
             WHERE x.artifact_id = a.id AND t.name IN ({}))",
            placeholders(filter.tags.len())
        ));
        params.extend(filter.tags.iter().cloned().map(Value::Text));
    }
    if omit != Some(Facet::Sources) && !filter.sources.is_empty() {
        parts.push(format!(
            "a.source_id IN ({})",
            placeholders(filter.sources.len())
        ));
        params.extend(filter.sources.iter().cloned().map(Value::Text));
    }
    if let Some(min) = filter.trust_min {
        parts.push("a.trust_score >= ?".to_owned());
        params.push(Value::Integer(i64::from(min)));
    }
    if omit != Some(Facet::Tiers) && !filter.tiers.is_empty() {
        parts.push(format!("a.tier IN ({})", placeholders(filter.tiers.len())));
        for tier in &filter.tiers {
            params.push(Value::Text(tier_name(*tier)?));
        }
    }
    if filter.has_findings {
        parts.push("COALESCE(json_array_length(a.findings), 0) > 0".to_owned());
    }
    if let Some(text) = filter
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let like = escape_like(text);
        parts.push(format!(
            "(a.name LIKE ? ESCAPE '\\' OR a.description LIKE ? ESCAPE '\\' \
             OR {LABEL_SQL} LIKE ? ESCAPE '\\' \
             OR EXISTS (SELECT 1 FROM artifact_tags x JOIN tags t ON t.id = x.tag_id \
                        WHERE x.artifact_id = a.id AND t.name LIKE ? ESCAPE '\\'))"
        ));
        params.extend(std::iter::repeat_n(Value::Text(like), 4));
    }
    if let Some(scope) = scope.filter(|s| !s.clause.is_empty()) {
        parts.push(format!("({})", scope.clause));
        params.extend(scope.params.iter().cloned());
    }
    let clause = if parts.is_empty() {
        "1".to_owned()
    } else {
        parts.join(" AND ")
    };
    Ok((clause, params))
}

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `owner/repo` for a git URL, the basename for a local folder.
fn source_label(kind: &str, location: &str) -> String {
    if kind == "local" {
        return location
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(location)
            .to_owned();
    }
    let trimmed = location.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let path = trimmed.rsplit_once("://").map_or(trimmed, |(_, rest)| rest);
    let path = path
        .rsplit_once(':')
        .filter(|(host, _)| !host.contains('/'))
        .map_or(path, |(_, p)| p);
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.as_slice() {
        [.., owner, repo] => format!("{owner}/{repo}"),
        [only] => (*only).to_owned(),
        [] => location.to_owned(),
    }
}

fn parse_kind(name: String) -> Result<ArtifactKind> {
    Ok(serde_json::from_value(serde_json::Value::String(name))?)
}

/// Runs `query` against the library index.
pub(crate) fn list(conn: &Connection, query: &Query) -> Result<Page> {
    list_in(conn, query, None)
}

/// Like [`list`], restricted by `scope` too (rows, total and facets alike).
pub(crate) fn list_in(conn: &Connection, query: &Query, scope: Option<&Scope>) -> Result<Page> {
    let (clause, filter_params) = build(&query.filter, None, scope)?;
    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id WHERE {clause}"
        ),
        params_from_iter(filter_params.iter()),
        |r| r.get(0),
    )?;

    let (base_clause, base_params) = build(&Filter::default(), None, scope)?;
    let unfiltered: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id \
             WHERE {base_clause}"
        ),
        params_from_iter(base_params.iter()),
        |r| r.get(0),
    )?;

    let order = match query.sort {
        SortKey::Name => "a.name COLLATE NOCASE".to_owned(),
        SortKey::Kind => "a.kind".to_owned(),
        SortKey::Source => format!("{LABEL_SQL} COLLATE NOCASE"),
        SortKey::TagCount => {
            "(SELECT COUNT(*) FROM artifact_tags x WHERE x.artifact_id = a.id)".to_owned()
        }
        SortKey::ImportedAt => "a.imported_at".to_owned(),
        // NULLs (not audited) sort last whichever way the scores go.
        SortKey::Trust => "(a.trust_score IS NULL) ASC, a.trust_score".to_owned(),
        SortKey::Position => match scope.and_then(|s| s.manual_collection) {
            Some(id) => format!(
                "(SELECT position FROM collection_items \
                  WHERE collection_id = {id} AND artifact_id = a.id)"
            ),
            None => "a.name COLLATE NOCASE".to_owned(),
        },
    };
    let direction = if query.descending { "DESC" } else { "ASC" };
    let mut params = filter_params;
    params.push(Value::Integer(i64::from(query.limit.min(MAX_LIMIT))));
    params.push(Value::Integer(i64::from(query.offset)));
    // The page is cut in the inner query, so the JSON diagnostic counts only run on its rows.
    let sql = format!(
        "SELECT p.id, p.kind, p.name, p.description, p.imported_at, p.source_id, p.skind,
                p.location, p.branch,
                p.trust_score, p.tier, COALESCE(json_array_length(p.findings), 0),
                (SELECT COUNT(*) FROM json_each(p.artifact, '$.diagnostics')
                  WHERE json_extract(value, '$.severity') = 'error'),
                (SELECT COUNT(*) FROM json_each(p.artifact, '$.diagnostics')
                  WHERE json_extract(value, '$.severity') = 'warning'),
                (SELECT COUNT(*) FROM json_each(p.artifact, '$.files')),
                (SELECT COUNT(*) FROM json_each(p.artifact, '$.references')),
                (SELECT COUNT(*) FROM json_each(p.artifact, '$.references')
                  WHERE json_extract(value, '$.resolved') = 1)
         FROM (SELECT a.id, a.kind, a.name, a.description, a.imported_at, a.source_id, a.artifact,
                      a.trust_score, a.tier, a.findings, s.kind AS skind, s.location, s.branch
               FROM artifacts a JOIN sources s ON s.id = a.source_id
               WHERE {clause}
               ORDER BY {order} {direction}, a.id ASC
               LIMIT ? OFFSET ?) p"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<i64>>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, i64>(11)?,
            r.get::<_, i64>(12)?,
            r.get::<_, i64>(13)?,
            r.get::<_, i64>(14)?,
            r.get::<_, i64>(15)?,
            r.get::<_, i64>(16)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (
            id,
            kind,
            name,
            description,
            imported_at,
            sid,
            skind,
            location,
            branch,
            trust,
            tier,
            finding_count,
            errors,
            warnings,
            files,
            references,
            resolved,
        ) = row?;
        items.push(LibraryItem {
            id,
            kind: parse_kind(kind)?,
            name,
            description,
            source: SourceSummary {
                id: sid,
                label: source_label(&skind, &location),
                kind: skind,
                branch,
            },
            tags: Vec::new(),
            imported_at,
            error_count: count(errors),
            warning_count: count(warnings),
            file_count: count(files),
            reference_count: count(references),
            resolved_references: count(resolved),
            trust_score: trust.and_then(|t| u8::try_from(t).ok()),
            tier: tier
                .map(|t| serde_json::from_value(serde_json::Value::String(t)))
                .transpose()?,
            finding_count: count(finding_count),
        });
    }
    drop(stmt);
    attach_tags(conn, &mut items)?;
    Ok(Page {
        items,
        total: count(total),
        unfiltered_total: count(unfiltered),
        facets: facets(conn, &query.filter, scope)?,
    })
}

fn attach_tags(conn: &Connection, items: &mut [LibraryItem]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "SELECT x.artifact_id, t.name, t.color FROM artifact_tags x JOIN tags t ON t.id = x.tag_id
         WHERE x.artifact_id IN ({}) ORDER BY t.name COLLATE NOCASE",
        placeholders(items.len())
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(items.iter().map(|i| i.id.as_str())), |r| {
        Ok((
            r.get::<_, String>(0)?,
            TagChip {
                name: r.get(1)?,
                color: r.get(2)?,
            },
        ))
    })?;
    let mut by_artifact: HashMap<String, Vec<TagChip>> = HashMap::new();
    for row in rows {
        let (id, chip) = row?;
        by_artifact.entry(id).or_default().push(chip);
    }
    for item in items {
        item.tags = by_artifact.remove(&item.id).unwrap_or_default();
    }
    Ok(())
}

fn facets(conn: &Connection, filter: &Filter, scope: Option<&Scope>) -> Result<Facets> {
    // Kinds.
    let (clause, params) = build(filter, Some(Facet::Types), scope)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT a.kind, COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id
         WHERE {clause} GROUP BY a.kind"
    ))?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut kinds: Vec<KindCount> = Vec::new();
    for row in rows {
        let (kind, n) = row?;
        kinds.push(KindCount {
            kind: parse_kind(kind)?,
            count: count(n),
        });
    }
    for kind in &filter.types {
        if !kinds.iter().any(|k| k.kind == *kind) {
            kinds.push(KindCount {
                kind: *kind,
                count: 0,
            });
        }
    }
    kinds.sort_by_key(|k| k.kind);
    drop(stmt);

    // Tags.
    let (clause, params) = build(filter, Some(Facet::Tags), scope)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT t.name, t.color, COUNT(f.id) FROM tags t
         LEFT JOIN artifact_tags x ON x.tag_id = t.id
         LEFT JOIN (SELECT a.id FROM artifacts a JOIN sources s ON s.id = a.source_id
                    WHERE {clause}) f ON f.id = x.artifact_id
         GROUP BY t.id ORDER BY t.name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
        Ok(TagCount {
            name: r.get(0)?,
            color: r.get(1)?,
            count: count(r.get(2)?),
        })
    })?;
    let tags = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    // Sources.
    let (clause, params) = build(filter, Some(Facet::Sources), scope)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.kind, s.location, s.branch, COUNT(f.id) FROM sources s
         LEFT JOIN (SELECT a.id, a.source_id FROM artifacts a JOIN sources s ON s.id = a.source_id
                    WHERE {clause}) f ON f.source_id = s.id
         GROUP BY s.id"
    ))?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
        let kind: String = r.get(1)?;
        let location: String = r.get(2)?;
        Ok(SourceCount {
            source: SourceSummary {
                id: r.get(0)?,
                label: source_label(&kind, &location),
                kind,
                branch: r.get(3)?,
            },
            count: count(r.get(4)?),
        })
    })?;
    let mut sources = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    sources.sort_by(|a, b| {
        (a.source.label.to_lowercase(), &a.source.id)
            .cmp(&(b.source.label.to_lowercase(), &b.source.id))
    });

    // Tiers.
    let (clause, params) = build(filter, Some(Facet::Tiers), scope)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT a.tier, COUNT(*) FROM artifacts a JOIN sources s ON s.id = a.source_id
         WHERE {clause} AND a.tier IS NOT NULL GROUP BY a.tier"
    ))?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut tiers: Vec<TierCount> = Vec::new();
    for row in rows {
        let (name, n) = row?;
        tiers.push(TierCount {
            tier: serde_json::from_value(serde_json::Value::String(name))?,
            count: count(n),
        });
    }
    for tier in &filter.tiers {
        if !tiers.iter().any(|t| t.tier == *tier) {
            tiers.push(TierCount {
                tier: *tier,
                count: 0,
            });
        }
    }
    tiers.sort_by_key(|t| t.tier);
    Ok(Facets {
        kinds,
        tags,
        sources,
        tiers,
    })
}
