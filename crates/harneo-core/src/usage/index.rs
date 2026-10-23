//! The incremental indexer: streams each `.jsonl` log under a root and records the names of
//! what sessions used. A file is re-read only from where it stopped, or from the start when it
//! shrank. Symlinks are never followed, and a line over 4 MiB is skipped.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use super::parse;
use crate::error::{Error, Result};

/// Lines longer than this are skipped (and counted as failed).
pub(crate) const MAX_LINE: usize = 4 * 1024 * 1024;

/// What one index run did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct IndexReport {
    /// Log files seen under the root.
    pub files: u32,
    /// Log files that were read (new or changed).
    pub files_read: u32,
    /// Uses not seen before.
    pub new_uses: u32,
    /// Lines that were not JSON objects or were too long, in the files read.
    pub failed_lines: u32,
    /// Log files or folders that could not be read (permissions, vanished mid-run) and were
    /// skipped; the rest was indexed.
    pub skipped_files: u32,
    /// Files whose last line had no newline yet, left for the next run.
    pub unfinished_lines: u32,
}

/// How far an index run is, sent while it runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct IndexProgress {
    pub files_done: u32,
    pub files_total: u32,
    pub new_uses: u32,
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        source,
    }
}

/// Every `.jsonl` file under `root`, without following symlinks, and the number of folders that
/// could not be read. A missing root is no files.
fn log_files(root: &Path) -> (Vec<PathBuf>, u32) {
    let mut files = Vec::new();
    let mut skipped = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|e| e == "jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    (files, skipped)
}

/// Reads one line without letting a huge one fill memory: returns `Some(false)` after an
/// over-long line was discarded, `Some(true)` with a line in `buf`, `None` at the end.
/// `complete` is false when the data ended without a newline.
fn next_line(
    reader: &mut impl BufRead,
    buf: &mut Vec<u8>,
    consumed: &mut u64,
    complete: &mut bool,
) -> std::io::Result<Option<bool>> {
    buf.clear();
    let mut too_long = false;
    let mut any = false;
    *complete = false;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            break;
        }
        any = true;
        let (take, found) = match chunk.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        if !too_long {
            if buf.len() + take > MAX_LINE + 1 {
                too_long = true;
                buf.clear();
            } else {
                buf.extend_from_slice(&chunk[..take]);
            }
        }
        reader.consume(take);
        *consumed += take as u64;
        if found {
            *complete = true;
            break;
        }
    }
    if !any {
        return Ok(None);
    }
    Ok(Some(!too_long))
}

/// How many leading bytes identify a file's content.
const HEAD_BYTES: u64 = 4096;

/// FNV-1a over the first `min(len, HEAD_BYTES)` bytes of `path`; stable across releases.
fn head_hash(path: &Path, len: u64) -> std::io::Result<i64> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(len.min(HEAD_BYTES))
        .read_to_end(&mut bytes)?;
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    Ok(hash as i64)
}

struct Known {
    size: u64,
    mtime: i64,
    offset: u64,
    head_hash: i64,
}

fn known(conn: &Connection, path: &str) -> Result<Option<Known>> {
    let mut stmt = conn
        .prepare_cached("SELECT size, mtime, offset, head_hash FROM log_files WHERE path = ?1")?;
    let mut rows = stmt.query(params![path])?;
    match rows.next()? {
        Some(row) => Ok(Some(Known {
            size: row.get::<_, i64>(0)?.max(0) as u64,
            mtime: row.get(1)?,
            offset: row.get::<_, i64>(2)?.max(0) as u64,
            head_hash: row.get(3)?,
        })),
        None => Ok(None),
    }
}

/// Indexes one file from `start`.
fn read_file(conn: &Connection, path: &Path, start: u64) -> Result<FileRead> {
    let session_fallback = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut file = File::open(path).map_err(|e| io_error(path, e))?;
    file.seek(SeekFrom::Start(start))
        .map_err(|e| io_error(path, e))?;
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let (mut buf, mut offset) = (Vec::new(), start);
    let (mut new_uses, mut failed, mut parsed) = (0u32, 0u32, 0u32);
    let mut unfinished = false;
    let mut insert = conn.prepare_cached(
        "INSERT OR IGNORE INTO uses (session_id, day, project_path, kind, name)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    loop {
        let mut consumed = 0u64;
        let mut complete = false;
        let Some(ok) = next_line(&mut reader, &mut buf, &mut consumed, &mut complete)
            .map_err(|e| io_error(path, e))?
        else {
            break;
        };
        if !complete {
            // A line still being written: leave it for the next run.
            unfinished = true;
            break;
        }
        offset += consumed;
        if !ok {
            failed += 1;
            continue;
        }
        let Ok(line) = std::str::from_utf8(&buf) else {
            failed += 1;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        if parse::is_broken(line) {
            failed += 1;
            continue;
        }
        parsed += 1;
        for u in parse::uses_in_line(line, &session_fallback) {
            let kind = serde_json::to_value(u.kind)?;
            new_uses += insert.execute(params![
                u.session_id,
                u.day,
                u.project_path,
                kind.as_str().unwrap_or_default(),
                u.name
            ])? as u32;
        }
    }
    Ok(FileRead {
        new_uses,
        failed,
        parsed,
        offset,
        unfinished,
    })
}

/// What reading one file found.
struct FileRead {
    new_uses: u32,
    failed: u32,
    parsed: u32,
    offset: u64,
    unfinished: bool,
}

/// Indexes `path` if it changed; returns what was read, or none when it was unchanged.
fn index_file(conn: &mut Connection, path: &Path) -> Result<Option<FileRead>> {
    let meta = fs::metadata(path).map_err(|e| io_error(path, e))?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    let key = path.to_string_lossy().into_owned();
    let old = known(conn, &key)?;
    if old
        .as_ref()
        .is_some_and(|k| k.size == size && k.mtime == mtime)
    {
        return Ok(None);
    }
    let start = match &old {
        Some(k)
            if size >= k.size
                && (k.head_hash == 0
                    || head_hash(path, k.offset).map_err(|e| io_error(path, e))?
                        == k.head_hash) =>
        {
            k.offset
        }
        _ => 0,
    };
    let tx = conn.transaction()?;
    let read = read_file(&tx, path, start)?;
    let (prev_parsed, prev_failed) = match (&old, start) {
        (Some(_), s) if s > 0 => tx
            .query_row(
                "SELECT parsed_lines, failed_lines FROM log_files WHERE path = ?1",
                params![key],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
            )
            .unwrap_or((0, 0)),
        _ => (0, 0),
    };
    let hash = head_hash(path, read.offset).map_err(|e| io_error(path, e))?;
    tx.execute(
        "INSERT INTO log_files (path, size, mtime, offset, parsed_lines, failed_lines, head_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(path) DO UPDATE SET size = ?2, mtime = ?3, offset = ?4,
             parsed_lines = ?5, failed_lines = ?6, head_hash = ?7",
        params![
            key,
            size as i64,
            mtime,
            read.offset as i64,
            prev_parsed + i64::from(read.parsed),
            prev_failed + i64::from(read.failed),
            hash
        ],
    )?;
    tx.commit()?;
    Ok(Some(read))
}

/// Runs the indexer over `logs_root`, reporting progress after each file. A file or folder that
/// cannot be read is skipped and counted; only index database errors stop the run.
pub(crate) fn run(
    conn: &mut Connection,
    logs_root: &Path,
    progress: &mut dyn FnMut(IndexProgress),
) -> Result<IndexReport> {
    let (files, skipped_dirs) = log_files(logs_root);
    let total = files.len() as u32;
    let mut report = IndexReport {
        files: total,
        skipped_files: skipped_dirs,
        ..IndexReport::default()
    };
    progress(IndexProgress {
        files_done: 0,
        files_total: total,
        new_uses: 0,
    });
    for (n, path) in files.iter().enumerate() {
        match index_file(conn, path) {
            Ok(Some(read)) => {
                report.files_read += 1;
                report.new_uses += read.new_uses;
                report.failed_lines += read.failed;
                report.unfinished_lines += u32::from(read.unfinished);
            }
            Ok(None) => {}
            Err(Error::Io { .. }) => report.skipped_files += 1,
            Err(e) => return Err(e),
        }
        progress(IndexProgress {
            files_done: n as u32 + 1,
            files_total: total,
            new_uses: report.new_uses,
        });
    }
    Ok(report)
}
