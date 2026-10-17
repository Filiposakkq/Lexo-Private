//! The file helpers of `core/lexo/util.py`: atomic writes, rotating backups, CSV files, the search matcher.
//! Every write goes through a temporary file in the same folder and a rename, so an interruption can never leave a half-written file.
use crate::clock::Clock;
use crate::csvio;
use crate::messages::{msg, Message};
use fancy_regex::Regex;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The test of a search box (`text_matcher`).
pub type Matcher = Box<dyn Fn(&str) -> bool>;

/// A write did not happen. `reason` is a short, user-presentable explanation.
#[derive(Debug, PartialEq)]
pub struct WriteFailed {
    pub path: PathBuf,
    pub reason: Box<Message>,
}

impl std::fmt::Display for WriteFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), self.reason.text())
    }
}

fn failed(path: &Path, reason: Message) -> WriteFailed {
    WriteFailed { path: path.to_path_buf(), reason: Box::new(reason) }
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn mtime(path: &Path) -> Option<f64> {
    fs::metadata(path).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs_f64())
}

/// The names `<name>.*.bak` in `dir`, sorted.
fn backups_of(dir: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.");
    let mut found: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.len() >= prefix.len() + 4 && n.starts_with(&prefix) && n.ends_with(".bak")
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => Vec::new(),
    };
    found.sort();
    found
}

fn stamp(t: chrono::NaiveDateTime) -> String {
    format!("{}-{:06}", t.format("%Y%m%d-%H%M%S"), t.and_utc().timestamp_subsec_micros())
}

/// Copy `path` into `backup_dir` as `<name>.<timestamp>.bak`. Skips when the newest backup of this file is younger than `min_interval` seconds, so a burst of edits
/// does not produce a burst of backups. Returns the backup path or `None`. The copy keeps the time of the original, like `shutil.copy2`.
pub fn make_backup(path: &Path, backup_dir: &Path, keep: usize, min_interval: f64, clock: &dyn Clock) -> Option<PathBuf> {
    if !path.is_file() {
        return None;
    }
    fs::create_dir_all(backup_dir).ok()?;
    let name = file_name(path);
    let existing = backups_of(backup_dir, &name);
    if let Some(last) = existing.last() {
        if min_interval != 0.0 && mtime(last).is_some_and(|m| clock.epoch() - m < min_interval) {
            return None;
        }
    }
    let mut now = clock.now_local();
    let mut dest = backup_dir.join(format!("{name}.{}.bak", stamp(now)));
    while dest.exists() {
        // the Windows clock can repeat within one tick: never overwrite a backup
        now += chrono::Duration::microseconds(1); // keeps the names sortable by time
        dest = backup_dir.join(format!("{name}.{}.bak", stamp(now)));
    }
    fs::copy(path, &dest).ok()?;
    if let Ok(t) = fs::metadata(path).and_then(|m| m.modified()) {
        if let Ok(f) = OpenOptions::new().write(true).open(&dest) {
            let _ = f.set_modified(t);
        }
    }
    let all = backups_of(backup_dir, &name);
    if keep > 0 && all.len() > keep {
        for old in &all[..all.len() - keep] {
            let _ = fs::remove_file(old);
        }
    }
    Some(dest)
}

fn random_hex() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.write_u32(std::process::id());
    format!("{:016x}", h.finish())
}

/// A random lower-case hex string of `bytes` bytes (`secrets.token_hex`; made from the random keys of the standard library, not for secrets).
pub fn token_hex(bytes: usize) -> String {
    let mut out = String::new();
    while out.len() < bytes * 2 {
        out.push_str(&random_hex());
    }
    out.truncate(bytes * 2);
    out
}

/// Write via a temporary file in the same folder + rename. Fails with a readable reason.
pub fn atomic_write_bytes(path: &Path, data: &[u8], backup_dir: Option<&Path>, clock: &dyn Clock) -> Result<(), WriteFailed> {
    let folder_err = |e: std::io::Error| {
        if e.kind() == ErrorKind::PermissionDenied {
            failed(path, msg("no permission to write to the folder", &[]))
        } else {
            failed(path, msg("the folder cannot be used ({reason})", &[("reason", json!(e.to_string()))]))
        }
    };
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(folder_err)?;
    if let Some(b) = backup_dir {
        if path.exists() {
            make_backup(path, b, 20, 300.0, clock);
        }
    }
    let tmp = parent.join(format!("{}.{}.tmp", file_name(path), &random_hex()[..8]));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(folder_err)?;
    let result = (|| -> Result<(), WriteFailed> {
        let other = |e: std::io::Error| failed(path, Message::new("{reason}", None, [("reason".to_string(), json!(e.to_string()))].into_iter().collect()));
        file.write_all(data).map_err(other)?;
        file.flush().map_err(other)?;
        file.sync_all().map_err(other)?;
        drop(file);
        for attempt in 0..3 {
            match fs::rename(&tmp, path) {
                Ok(()) => return Ok(()),
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    if attempt == 2 {
                        return Err(failed(path, msg("the file is locked (open in Excel or another program?)", &[])));
                    }
                    std::thread::sleep(Duration::from_millis(150 * (attempt + 1)));
                }
                Err(e) => return Err(other(e)),
            }
        }
        Ok(())
    })();
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// `newline` (when given) turns every line end of the text into it first.
pub fn atomic_write_text(path: &Path, text: &str, newline: Option<&str>, backup_dir: Option<&Path>, clock: &dyn Clock) -> Result<(), WriteFailed> {
    match newline {
        Some(nl) => atomic_write_bytes(path, text.replace("\r\n", "\n").replace('\n', nl).as_bytes(), backup_dir, clock),
        None => atomic_write_bytes(path, text.as_bytes(), backup_dir, clock),
    }
}

/// Excel-friendly CSV: UTF-8 with BOM, `;` delimiter, CRLF: the format the old tool wrote.
pub fn write_csv(path: &Path, rows: &[Vec<String>], backup_dir: Option<&Path>, clock: &dyn Clock) -> Result<(), WriteFailed> {
    let mut data = vec![0xEF, 0xBB, 0xBF];
    data.extend(csvio::csv_text(rows, ';').into_bytes());
    atomic_write_bytes(path, &data, backup_dir, clock)
}

/// Read a CSV (`,` or `;`, with or without BOM). A missing file gives no rows; any other read error is returned.
pub fn read_rows(path: &Path) -> std::io::Result<Vec<Vec<String>>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = fs::read(path)?;
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    Ok(csvio::read_rows_text(&String::from_utf8_lossy(body)))
}

/// A function text -> bool for a search box: the query anywhere in the text (case does not matter), or, with `whole`, only as a whole word (`Sage` finds "Sage's"
/// and "Sage," but not "Massage"). `None` for an empty query.
pub fn text_matcher(query: &str, whole: bool) -> Option<Matcher> {
    let q = crate::util::py_strip(query);
    if q.is_empty() {
        return None;
    }
    if !whole {
        let low = q.to_lowercase();
        return Some(Box::new(move |text: &str| text.to_lowercase().contains(&low)));
    }
    let rx = Regex::new(&format!(r"(?i)(?<!\w){}(?!\w)", fancy_regex::escape(q))).ok()?;
    Some(Box::new(move |text: &str| rx.is_match(text).unwrap_or(false)))
}
