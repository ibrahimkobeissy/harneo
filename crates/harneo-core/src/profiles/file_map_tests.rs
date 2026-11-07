//! The map and the fits in a profile file (format 2), and `clone_profile`.

use serde_json::{json, Value};

use super::file_tests::{edit, empty, exported, library_from, source, Lib};
use super::*;
use crate::harnmap::{FitVerdict, MapDoc, PlacedBy, Placement, Verdict, Zone};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

type Change = Box<dyn Fn(&mut Value)>;

const BASIS: &str = r#"{"profileVersion":1,"artifacts":[]}"#;

fn placement(artifact: &str, zone: Zone, note: &str, pinned: bool) -> Placement {
    Placement {
        artifact: artifact.to_owned(),
        zone,
        order: 0,
        note: note.to_owned(),
        why: "because".to_owned(),
        role: "r".to_owned(),
        when: "w".to_owned(),
        hands_off: "h".to_owned(),
        by: if pinned {
            PlacedBy::User
        } else {
            PlacedBy::Claude
        },
        pinned,
    }
}

fn add_draw(lib: &Lib, profile: ProfileId, no: u32, doc: &MapDoc) -> TestResult {
    lib.library.store.conn.execute(
        "INSERT INTO harnmap_draws (profile_id, no, created_at, model, duration_ms, description,
             category, map, basis, restored_from)
         VALUES (?1, ?2, 100, 'opus', 5000, ?3, 'software', ?4, ?5, NULL)",
        params![
            profile,
            no,
            format!("draw {no}"),
            serde_json::to_string(doc)?,
            BASIS
        ],
    )?;
    Ok(())
}

fn add_fit(lib: &Lib, profile: ProfileId, candidate: &str, conflicts: &[String]) -> TestResult {
    let verdict = json!({
        "verdict": "conflicts",
        "zone": {"type": "floor"},
        "why": "overlaps",
        "change": null,
        "conflictsWith": conflicts,
        "quotes": [
            {"artifact": "candidate", "file": "a.md", "line": 1, "text": "x"},
            {"artifact": conflicts.first().cloned().unwrap_or_default(), "file": "b.md",
             "line": 2, "text": "y"}
        ],
        "droppedQuotes": 0,
        "missingMcp": [],
        "candidate": {"key": candidate, "label": "cand", "kind": "skill", "source": "Library"},
        "model": "opus",
        "durationMs": 7,
        "createdAt": 200
    });
    let parsed: FitVerdict = serde_json::from_value(verdict)?;
    assert_eq!(parsed.verdict, Verdict::Conflicts);
    let basis = json!({
        "profile": serde_json::from_str::<Value>(BASIS)?,
        "candidate_digest": "cd",
        "source": "s"
    });
    lib.library.store.conn.execute(
        "INSERT INTO harnfit_fits (profile_id, candidate, created_at, model, duration_ms, verdict,
             basis) VALUES (?1, ?2, 200, 'opus', 7, ?3, ?4)",
        params![
            profile,
            candidate,
            serde_json::to_string(&parsed)?,
            basis.to_string()
        ],
    )?;
    Ok(())
}

/// A library with the heavy profile, two draws (the second pins and notes), two fits and a
/// Custom category.
struct Mapped {
    lib: Lib,
    profile: ProfileId,
    skill: String,
    helper: String,
    bytes: Vec<u8>,
}

fn mapped() -> std::result::Result<Mapped, Box<dyn std::error::Error>> {
    let lib = library_from(source()?)?;
    let (profile, _, _) = exported(&lib)?;
    lib.library
        .set_category(profile, CategoryId::Custom, Some("a bakery"))?;
    let skill = lib.id(ArtifactKind::Skill, "a")?;
    let helper = lib.id(ArtifactKind::Agent, "helper")?;
    let mut first = MapDoc::from_template(CategoryId::Custom);
    first.placements = vec![placement(&skill, Zone::Toolbelt, "", false)];
    add_draw(&lib, profile, 1, &first)?;
    let stage = first.stages.first().ok_or("a stage")?.id.clone();
    let mut second = first.clone();
    second.placements = vec![
        placement(&skill, Zone::Stage { id: stage }, "mixes", true),
        placement(&helper, Zone::Floor, "", false),
    ];
    second.hidden = vec![lib.id(ArtifactKind::Mcp, "api")?];
    add_draw(&lib, profile, 2, &second)?;
    add_fit(
        &lib,
        profile,
        &format!("lib:{helper}"),
        std::slice::from_ref(&skill),
    )?;
    add_fit(&lib, profile, "src:owner/repo@abc:skill/other", &[])?;
    let bytes = lib
        .library
        .export_profile(profile, None, ExportOptions::default())?;
    Ok(Mapped {
        lib,
        profile,
        skill,
        helper,
        bytes,
    })
}

#[test]
fn a_round_trip_keeps_draws_pins_notes_fits_and_the_category() -> TestResult {
    let one = mapped()?;
    let file: Value = serde_json::from_slice(&one.bytes)?;
    assert_eq!(file["formatVersion"], 2);
    assert_eq!(file["profile"]["category"], "custom");
    assert_eq!(file["profile"]["categoryNote"], "a bakery");
    assert_eq!(file["harnmap"]["draws"].as_array().map(Vec::len), Some(2));
    assert_eq!(file["fits"].as_array().map(Vec::len), Some(2));
    // Inside the file artifacts are keys, never ids.
    let first = &file["harnmap"]["draws"][1]["map"]["placements"][0]["artifact"];
    assert_eq!(first["kind"], "skill");
    assert_eq!(first["name"], "a");
    assert!(first["digest"].is_string());
    let carried = format!("{}{}", file["harnmap"], file["fits"]);
    assert!(!carried.contains(&one.skill) && !carried.contains(&one.helper));

    let mut two = empty()?;
    let report = two.library.import_profile_file_commit(&one.bytes, None)?;
    assert_eq!((report.map_draws, report.fits), (2, 2));
    let view = two.library.profile(report.profile_id, None)?;
    assert_eq!(view.category, CategoryId::Custom);
    assert_eq!(view.category_note.as_deref(), Some("a bakery"));

    let map = two.library.harnmap(report.profile_id)?;
    assert!(map.drawn && map.no == 2 && map.draw_count == 2);
    assert!(map.stale.is_none(), "a fresh import is not out of date");
    let skill = two.id(ArtifactKind::Skill, "a")?;
    let helper = two.id(ArtifactKind::Agent, "helper")?;
    let placed: Vec<(&str, bool, &str)> = map
        .doc
        .placements
        .iter()
        .map(|p| (p.artifact.as_str(), p.pinned, p.note.as_str()))
        .collect();
    assert_eq!(
        placed,
        vec![
            (skill.as_str(), true, "mixes"),
            (helper.as_str(), false, "")
        ]
    );
    assert_eq!(map.doc.hidden, vec![two.id(ArtifactKind::Mcp, "api")?]);
    assert_eq!(two.library.harnmap_history(report.profile_id)?.len(), 2);

    let fits = two.library.fits(report.profile_id)?;
    assert_eq!(fits.len(), 2);
    let keys: Vec<String> = fits.iter().map(|f| f.candidate.key.clone()).collect();
    assert!(keys.contains(&format!("lib:{helper}")), "{keys:?}");
    assert!(keys.contains(&"src:owner/repo@abc:skill/other".to_owned()));
    let view = two
        .library
        .harnfit_get(report.profile_id, &format!("lib:{helper}"))?
        .ok_or("the fit")?;
    assert_eq!(view.verdict.conflicts_with, vec![skill.clone()]);
    assert_eq!(view.verdict.quotes.len(), 2);
    assert_eq!(view.verdict.quotes[1].artifact, skill);
    assert_eq!(view.verdict.quotes[0].artifact, "candidate");
    assert_eq!(view.verdict.created_at, 200);
    Ok(())
}

#[test]
fn the_options_decide_what_the_file_carries() -> TestResult {
    let one = mapped()?;
    let export = |include_history, include_fits| {
        one.lib.library.export_profile(
            one.profile,
            None,
            ExportOptions {
                include_history,
                include_fits,
            },
        )
    };
    let bare = export(false, false)?;
    let file: Value = serde_json::from_slice(&bare)?;
    assert_eq!(file["harnmap"]["draws"].as_array().map(Vec::len), Some(1));
    assert_eq!(file["harnmap"]["draws"][0]["no"], 2);
    assert!(file.get("fits").is_none());
    assert!(bare.len() < one.bytes.len());

    let mut two = empty()?;
    let report = two.library.import_profile_file_commit(&bare, None)?;
    assert_eq!((report.map_draws, report.fits), (1, 0));
    let map = two.library.harnmap(report.profile_id)?;
    assert!(map.drawn && map.no == 2 && map.draw_count == 1);
    assert!(two.library.fits(report.profile_id)?.is_empty());

    let fits_only = export(false, true)?;
    let value: Value = serde_json::from_slice(&fits_only)?;
    assert_eq!(value["fits"].as_array().map(Vec::len), Some(2));
    Ok(())
}

#[test]
fn a_profile_with_no_map_exports_none() -> TestResult {
    let lib = library_from(source()?)?;
    let (profile, _, bytes) = exported(&lib)?;
    let file: Value = serde_json::from_slice(&bytes)?;
    assert!(file.get("harnmap").is_none() && file.get("fits").is_none());
    let mut two = empty()?;
    let report = two.library.import_profile_file_commit(&bytes, None)?;
    assert_eq!((report.map_draws, report.fits), (0, 0));
    assert!(!two.library.harnmap(report.profile_id)?.drawn);
    assert!(lib.library.harnmap(profile).is_ok());
    Ok(())
}

#[test]
fn a_version_1_file_imports_with_no_map() -> TestResult {
    let one = mapped()?;
    let v1 = edit(&one.bytes, |v| {
        v["formatVersion"] = Value::from(1);
        if let Some(p) = v["profile"].as_object_mut() {
            p.remove("category");
            p.remove("categoryNote");
        }
    })?;
    let two = empty()?;
    let preview = two.library.import_profile_file(&v1)?;
    assert_eq!(preview.profile.name, "Heavy");
    let mut two = two;
    let report = two.library.import_profile_file_commit(&v1, None)?;
    assert_eq!((report.map_draws, report.fits), (0, 0));
    let view = two.library.profile(report.profile_id, None)?;
    assert_eq!(view.category, CategoryId::Software);
    assert!(!two.library.harnmap(report.profile_id)?.drawn);
    Ok(())
}

fn refused(bytes: &[u8]) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let two = empty()?;
    let message = two
        .library
        .import_profile_file(bytes)
        .err()
        .map(|e| e.to_string())
        .ok_or("the file was accepted")?;
    // Nothing is stored by a refused commit either.
    let mut two = two;
    assert!(two.library.import_profile_file_commit(bytes, None).is_err());
    assert!(two.library.profiles()?.is_empty());
    Ok(message)
}

#[test]
fn a_malformed_map_part_is_refused() -> TestResult {
    let one = mapped()?;
    let cases: Vec<Change> = vec![
        // A placement on a stage the map does not have.
        Box::new(|v| {
            v["harnmap"]["draws"][1]["map"]["placements"][0]["zone"] =
                json!({"type": "stage", "id": "nope"});
        }),
        // A map that is not a map.
        Box::new(|v| v["harnmap"]["draws"][1]["map"] = json!("hello")),
        // An artifact reference that is not a key.
        Box::new(|v| v["harnmap"]["draws"][1]["map"]["placements"][0]["artifact"] = json!(7)),
        // Two drawings with one number.
        Box::new(|v| v["harnmap"]["draws"][0]["no"] = json!(2)),
        // Too many drawings.
        Box::new(|v| {
            let draw = v["harnmap"]["draws"][1].clone();
            v["harnmap"]["draws"] = Value::Array(vec![draw; 501]);
        }),
        // A fit candidate that is neither a key nor a preview key.
        Box::new(|v| v["fits"][0]["candidate"] = json!("lib:abc")),
        Box::new(|v| v["fits"][1]["candidate"] = json!("lib:abc")),
        // A fit verdict that is not a verdict.
        Box::new(|v| v["fits"][0]["verdict"]["verdict"] = json!("maybe")),
        // Too many fits.
        Box::new(|v| {
            let fit = v["fits"][1].clone();
            v["fits"] = Value::Array(vec![fit; 501]);
        }),
    ];
    for (i, change) in cases.iter().enumerate() {
        let bad = edit(&one.bytes, |v| change(v))?;
        let message = refused(&bad)?;
        assert!(
            message.contains("map part is malformed"),
            "case {i}: {message}"
        );
    }
    Ok(())
}

#[test]
fn unmapped_references_are_dropped_and_long_texts_cut() -> TestResult {
    let one = mapped()?;
    let ghost = json!({"kind": "skill", "name": "ghost", "digest": "zz"});
    let long = "x".repeat(5000);
    let changed = edit(&one.bytes, |v| {
        let map = &mut v["harnmap"]["draws"][1]["map"];
        if let Some(p) = map["placements"].as_array_mut() {
            p.push(json!({"artifact": ghost}));
        }
        map["placements"][0]["note"] = json!(long);
        if let Some(h) = map["hidden"].as_array_mut() {
            h.push(ghost.clone());
        }
        // The lib fit of an artifact nobody has is dropped; its conflicts to ghosts too.
        v["fits"][0]["candidate"] = ghost.clone();
        v["fits"][1]["verdict"]["conflictsWith"] = json!([ghost]);
        v["fits"][1]["verdict"]["why"] = json!(long);
    })?;
    let mut two = empty()?;
    let report = two.library.import_profile_file_commit(&changed, None)?;
    assert_eq!((report.map_draws, report.fits), (2, 1));
    let map = two.library.harnmap(report.profile_id)?;
    assert_eq!(map.doc.placements.len(), 2);
    assert_eq!(map.doc.placements[0].note.chars().count(), 60);
    assert_eq!(map.doc.hidden.len(), 1);
    let fit = two
        .library
        .harnfit_get(report.profile_id, "src:owner/repo@abc:skill/other")?
        .ok_or("the preview fit is kept")?;
    assert!(fit.verdict.conflicts_with.is_empty());
    assert_eq!(fit.verdict.why.chars().count(), 2000);
    Ok(())
}

#[test]
fn an_import_into_a_library_that_has_the_artifacts_reuses_them() -> TestResult {
    let one = mapped()?;
    // A second library that already holds every artifact of the file.
    let mut two = library_from(source()?)?;
    let report = two.library.import_profile_file_commit(&one.bytes, None)?;
    assert!(report.imported.is_empty());
    assert_eq!((report.map_draws, report.fits), (2, 2));
    let helper = two.id(ArtifactKind::Agent, "helper")?;
    let map = two.library.harnmap(report.profile_id)?;
    assert!(map.doc.placements.iter().any(|p| p.artifact == helper));
    assert!(map.stale.is_none());
    Ok(())
}

#[test]
fn clone_copies_the_category_map_history_and_fits() -> TestResult {
    let one = mapped()?;
    let copy = one.lib.library.clone_profile(one.profile, "Copy")?;
    let view = one.lib.library.profile(copy, None)?;
    assert_eq!(view.category, CategoryId::Custom);
    assert_eq!(view.category_note.as_deref(), Some("a bakery"));
    let map = one.lib.library.harnmap(copy)?;
    assert!(map.drawn && map.no == 2 && map.draw_count == 2);
    assert_eq!(map.doc.placements.len(), 2);
    assert_eq!(map.doc.placements[0].artifact, one.skill);
    assert_eq!(one.lib.library.harnmap_history(copy)?.len(), 2);
    assert_eq!(one.lib.library.fits(copy)?.len(), 2);
    // The copies are independent.
    one.lib.library.harnmap_edit(
        copy,
        crate::harnmap::MapEdit::Hide {
            artifact: one.helper.clone(),
        },
    )?;
    let original = one.lib.library.harnmap(one.profile)?;
    assert!(!original.doc.hidden.contains(&one.helper));
    Ok(())
}
