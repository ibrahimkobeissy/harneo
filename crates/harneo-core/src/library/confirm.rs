//! Audit phase 2 in the library: which artifacts wait for Claude, the background job that asks
//! it, the verdict cache and the stored status. The judging itself is `audit::confirm`.
//!
//! A job reads everything it needs up front (the phase 1 audit of each stored copy, the
//! findings' context and the cached verdicts), so the slow Claude calls run without holding the
//! library. Each artifact's result is handed to a callback that stores it, so the library shows
//! `pending` only until that artifact is done.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{copy, records, ArtifactId, Library, Preview};
use crate::artifact::{Artifact, ArtifactKind, ScanOptions};
use crate::audit::confirm::{self, Candidate, Subject, Verdict};
use crate::audit::{self, Audit, AuditInput, Confirmation, Finding};
use crate::bundles::{model_of, ClaudeConfig};
use crate::error::{Error, Result};

/// The phase 2 state of one artifact, for the status list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ArtifactConfirmation {
    pub id: ArtifactId,
    /// `None` for an artifact that was never audited.
    pub confirmation: Option<Confirmation>,
    /// How many findings Claude dismissed.
    pub dismissed: u32,
}

/// How far a phase 2 run is: artifacts done of the total.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ConfirmProgress {
    pub done: u32,
    pub total: u32,
}

impl ConfirmProgress {
    pub fn new(done: usize, total: usize) -> Self {
        Self {
            done: u32::try_from(done).unwrap_or(u32::MAX),
            total: u32::try_from(total).unwrap_or(u32::MAX),
        }
    }
}

/// Which artifacts a phase 2 run covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmScope {
    /// Those waiting for Claude (after an import, an update or a re-audit).
    Pending,
    /// These artifacts, whatever their status.
    Ids(Vec<ArtifactId>),
    /// The whole library.
    All,
}

/// The phase 1 audit of a stored copy with the context phase 2 needs. Findings point at the
/// source paths, as the stored audit does.
pub(crate) struct Phase1 {
    pub audit: Audit,
    pub cands: Vec<Candidate>,
}

/// Audits the stored copy in `folder` (phase 1) and reads each finding's context.
pub(crate) fn phase1_of(
    artifact: &Artifact,
    folder: &Path,
    options: &ScanOptions,
) -> Result<Phase1> {
    let (view, to_source) = copy::stored_view(artifact)?;
    let input = AuditInput::read_stored(&view, folder, options);
    let mut audit = audit::audit_input(&input);
    let cands = confirm::candidates(&input, &audit.findings);
    for finding in &mut audit.findings {
        if let Some(source) = to_source.get(&finding.file) {
            finding.file.clone_from(source);
        }
    }
    Ok(Phase1 { audit, cands })
}

/// The cached verdicts for `cands`.
pub(crate) fn cached(conn: &Connection, cands: &[Candidate]) -> Result<HashMap<String, Verdict>> {
    let mut stmt =
        conn.prepare_cached("SELECT threat, reason FROM audit_verdicts WHERE key = ?1")?;
    let mut out = HashMap::new();
    for c in cands {
        let row: Option<(bool, String)> = stmt
            .query_row(params![c.key], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        if let Some((threat, reason)) = row {
            out.insert(c.key.clone(), Verdict { threat, reason });
        }
    }
    Ok(out)
}

/// Keeps the verdicts Claude gave under their keys.
fn store_verdicts(
    conn: &Connection,
    fresh: &[(String, Verdict)],
    model: &str,
    at: i64,
) -> Result<()> {
    for (key, verdict) in fresh {
        conn.execute(
            "INSERT INTO audit_verdicts (key, threat, reason, model, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (key) DO UPDATE SET threat = excluded.threat,
                reason = excluded.reason, model = excluded.model,
                created_at = excluded.created_at",
            params![key, verdict.threat, verdict.reason, model, at],
        )?;
    }
    Ok(())
}

fn kind_of(name: &str) -> Result<ArtifactKind> {
    Ok(serde_json::from_value(serde_json::Value::String(
        name.to_owned(),
    ))?)
}

/// One artifact in a job.
#[derive(Clone)]
struct Item {
    id: ArtifactId,
    subject: Subject,
    phase1: Audit,
    cands: Vec<Candidate>,
    cache: HashMap<String, Verdict>,
}

/// What a run decided for one artifact; [`Library::store_confirmation`] keeps it.
#[derive(Debug, Clone)]
pub struct ConfirmOutcome {
    pub id: ArtifactId,
    pub audit: Audit,
    fresh: Vec<(String, Verdict)>,
    model: String,
    at: i64,
}

/// Everything a phase 2 run needs, read from the library up front.
pub struct ConfirmJob {
    enabled: bool,
    config: ClaudeConfig,
    items: Vec<Item>,
}

impl ConfirmJob {
    /// How many artifacts the run covers.
    pub fn artifact_count(&self) -> usize {
        self.items.len()
    }

    /// Judges the artifacts one after another. `progress(done, total)` is called before the
    /// first and after each; `store` receives each result as soon as it is known. With phase 2
    /// off nothing is asked and every artifact is stored as `off`.
    pub fn run(
        &self,
        progress: &dyn Fn(usize, usize),
        store: &mut dyn FnMut(&ConfirmOutcome) -> Result<()>,
    ) -> Result<()> {
        self.run_until(progress, store, &|| false)
    }

    /// [`Self::run`] that stops before the next artifact once `cancelled()` is true.
    pub fn run_until(
        &self,
        progress: &dyn Fn(usize, usize),
        store: &mut dyn FnMut(&ConfirmOutcome) -> Result<()>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<()> {
        let total = self.items.len();
        progress(0, total);
        // Set once a call fails because of the account: no further call is made in this run.
        let mut blocked: Option<confirm::Blocked> = None;
        for (done, item) in self.items.iter().enumerate() {
            if cancelled() {
                return Ok(());
            }
            let at = records::now();
            let outcome = if self.enabled {
                let found = confirm::confirm_unless_blocked(
                    &self.config,
                    &item.subject,
                    item.phase1.clone(),
                    &item.cands,
                    &item.cache,
                    at,
                    blocked.as_ref(),
                );
                if blocked.is_none() {
                    blocked = confirm::blocked_by(&found.audit);
                }
                ConfirmOutcome {
                    id: item.id.clone(),
                    audit: found.audit,
                    fresh: found.fresh,
                    model: self.config.model.clone(),
                    at,
                }
            } else {
                ConfirmOutcome {
                    id: item.id.clone(),
                    audit: confirm::off(item.phase1.clone()),
                    fresh: Vec::new(),
                    model: self.config.model.clone(),
                    at,
                }
            };
            store(&outcome)?;
            progress(done + 1, total);
        }
        Ok(())
    }
}

impl Library {
    /// Whether phase 2 is on: the `audit.confirmWithClaude` setting, on unless set to `false`.
    pub fn confirm_enabled(&self) -> Result<bool> {
        records::confirm_enabled(&self.store.conn)
    }

    /// The phase 2 state of every audited artifact.
    pub fn confirmations(&self) -> Result<Vec<ArtifactConfirmation>> {
        let mut stmt = self
            .store
            .conn
            .prepare("SELECT id FROM artifacts ORDER BY name, id")?;
        let ids: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let audit = records::load_audit(&self.store.conn, &id)?.map(|(a, _)| a);
            out.push(ArtifactConfirmation {
                id,
                dismissed: audit
                    .as_ref()
                    .map_or(0, |a| u32::try_from(a.dismissed.len()).unwrap_or(u32::MAX)),
                confirmation: audit.and_then(|a| a.confirmation),
            });
        }
        Ok(out)
    }

    /// Reads what a phase 2 run over `scope` needs. Artifacts whose copy is missing are left
    /// out. Run the job with [`ConfirmJob::run`] (it does not need the library).
    pub fn prepare_confirmation(&self, scope: &ConfirmScope) -> Result<ConfirmJob> {
        let conn = &self.store.conn;
        let enabled = records::confirm_enabled(conn)?;
        let ids: Vec<String> = match scope {
            ConfirmScope::Ids(ids) => ids.clone(),
            ConfirmScope::All => {
                let mut stmt = conn.prepare("SELECT id FROM artifacts ORDER BY imported_at, id")?;
                let rows = stmt.query_map([], |r| r.get(0))?;
                rows.collect::<std::result::Result<_, _>>()?
            }
            ConfirmScope::Pending => {
                let mut stmt = conn.prepare(
                    "SELECT id FROM artifacts
                     WHERE json_extract(confirmation, '$.status') = 'pending'
                     ORDER BY imported_at, id",
                )?;
                let rows = stmt.query_map([], |r| r.get(0))?;
                rows.collect::<std::result::Result<_, _>>()?
            }
        };
        let config = self.confirm_config()?;
        let options = ScanOptions::default();
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for id in ids {
            if !seen.insert(id.clone()) {
                continue;
            }
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT kind, artifact FROM artifacts WHERE id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((kind, json)) = row else {
                if matches!(scope, ConfirmScope::Ids(_)) {
                    return Err(Error::NotInLibrary { id });
                }
                continue;
            };
            let folder = self.store.artifacts_dir().join(&id);
            if !folder.is_dir() {
                continue;
            }
            let artifact: Artifact = serde_json::from_str(&json)?;
            let phase1 = phase1_of(&artifact, &folder, &options)?;
            let cache = if enabled {
                cached(conn, &phase1.cands)?
            } else {
                HashMap::new()
            };
            items.push(Item {
                id,
                subject: Subject {
                    kind: kind_of(&kind)?,
                    name: artifact.name.clone(),
                    description: artifact.description.clone().unwrap_or_default(),
                },
                phase1: phase1.audit,
                cands: phase1.cands,
                cache,
            });
        }
        Ok(ConfirmJob {
            enabled,
            config,
            items,
        })
    }

    /// Reads what a phase 2 run over the flagged, not yet imported candidates of `preview`
    /// needs. Verdicts are cached under the names the stored copy will have, so the run after
    /// the import finds them. With phase 2 off the job is empty. Run it with
    /// [`ConfirmJob::run_until`] and keep each result with [`Self::store_preview_verdicts`].
    pub fn prepare_preview_confirmation(&self, preview: &Preview) -> Result<ConfirmJob> {
        let conn = &self.store.conn;
        let enabled = records::confirm_enabled(conn)?;
        let config = self.confirm_config()?;
        let mut items = Vec::new();
        if enabled {
            let options = ScanOptions::default();
            self.with_preview_root(preview, |root| {
                for c in &preview.candidates {
                    if c.already_in_library || c.audit.findings.is_empty() {
                        continue;
                    }
                    let input = AuditInput::read(&c.artifact, root, &options);
                    let phase1 = audit::audit_input(&input);
                    let to_stored: HashMap<String, String> =
                        copy::plan(&c.artifact)?.into_iter().collect();
                    let cands = confirm::candidates_keyed(&input, &phase1.findings, &|file| {
                        to_stored
                            .get(file)
                            .cloned()
                            .unwrap_or_else(|| file.to_owned())
                    });
                    let cache = cached(conn, &cands)?;
                    items.push(Item {
                        id: c.id.clone(),
                        subject: Subject {
                            kind: c.artifact.kind,
                            name: c.artifact.name.clone(),
                            description: c.artifact.description.clone().unwrap_or_default(),
                        },
                        phase1,
                        cands,
                        cache,
                    });
                }
                Ok(())
            })?;
        }
        Ok(ConfirmJob {
            enabled,
            config,
            items,
        })
    }

    /// Keeps the verdicts Claude gave for a preview candidate, in the shared cache.
    pub fn store_preview_verdicts(&self, outcome: &ConfirmOutcome) -> Result<()> {
        store_verdicts(&self.store.conn, &outcome.fresh, &outcome.model, outcome.at)
    }

    /// Keeps one artifact's phase 2 result and the verdicts Claude gave. An artifact removed
    /// in the meantime is skipped.
    pub fn store_confirmation(&self, outcome: &ConfirmOutcome) -> Result<()> {
        let tx = self.store.conn.unchecked_transaction()?;
        store_verdicts(&tx, &outcome.fresh, &outcome.model, outcome.at)?;
        records::update_audit(&tx, &outcome.id, &outcome.audit, outcome.at)?;
        tx.commit()?;
        Ok(())
    }

    /// Runs the `claude` CLI as `config` says from now on (the default is `claude` from the
    /// `PATH`); for callers that must point it elsewhere, such as tests with a fixture script.
    pub fn use_claude(&mut self, config: ClaudeConfig) {
        self.claude = config;
    }

    /// The Claude settings phase 2 runs with: the program, and the model from Settings.
    fn confirm_config(&self) -> Result<ClaudeConfig> {
        let mut config = self.claude.clone();
        config.model = model_of(self.settings_get("bundles.model")?.as_ref());
        Ok(config)
    }

    /// Audits a scanned folder (not a library copy) with both phases, for the CLI. Phase 2
    /// runs when the setting is on and `claude` is true; verdicts are cached in the library
    /// like the app's.
    pub fn audit_confirmed(
        &self,
        artifact: &Artifact,
        root: &Path,
        options: &ScanOptions,
        claude: bool,
    ) -> Result<Audit> {
        let input = AuditInput::read(artifact, root, options);
        let phase1 = audit::audit_input(&input);
        if !claude || !records::confirm_enabled(&self.store.conn)? {
            return Ok(confirm::off(phase1));
        }
        let cands = confirm::candidates(&input, &phase1.findings);
        let cache = cached(&self.store.conn, &cands)?;
        let config = self.confirm_config()?;
        let subject = Subject {
            kind: artifact.kind,
            name: artifact.name.clone(),
            description: artifact.description.clone().unwrap_or_default(),
        };
        let at = records::now();
        let got = confirm::confirm(&config, &subject, phase1, &cands, &cache, at);
        store_verdicts(&self.store.conn, &got.fresh, &config.model, at)?;
        Ok(got.audit)
    }

    /// Runs phase 2 over `scope` now, holding the library: the same as preparing a job and
    /// storing each result. Returns how many artifacts were covered. The app prepares the
    /// job first and runs it without the library; this is for callers with no UI.
    pub fn confirm_now(&self, scope: &ConfirmScope) -> Result<usize> {
        let job = self.prepare_confirmation(scope)?;
        job.run(&|_, _| {}, &mut |outcome| self.store_confirmation(outcome))?;
        Ok(job.artifact_count())
    }
}

/// `findings` of both lists, for comparing two audits' phase 1 content.
pub(crate) fn candidate_set(audit: &Audit) -> std::collections::HashSet<Finding> {
    audit
        .findings
        .iter()
        .chain(audit.dismissed.iter().map(|d| &d.finding))
        .cloned()
        .collect()
}
