//! Port of `core/lexo/snapshots.py`: snapshots of the translation folder: a zip of every `.rpy` file, kept in the local data folder (datadir.rs).
//!
//! Made automatically (at start when the last one is older than the configured interval), before bulk changes (replace, fixes, unify) and on request; restoring first
//! makes a snapshot of the current state, so a restore itself can be undone. Only `.rpy` files are saved (the manager's own data have their own backups). Snapshots older
//! than the configured count are deleted, the manual ones last. The zip is made with the deflate of this program: its bytes differ from the ones Python makes, its
//! content does not (and either program reads the other's).
use crate::clock::Clock;
use crate::fsutil::atomic_write_bytes;
use crate::messages::{msg, Message};
use crate::pyfmt::json_dumps;
use crate::util::sha1_hex;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const DIR_NAME: &str = "snapshots";
const SKIP_DIRS: [&str; 5] = ["_progress_data", ".lexo", "__pycache__", ".git", "backups"];
pub const MAX_BYTES: u64 = 400 * 1024 * 1024;

/// What a snapshot was made for (the keys of the name) and the label shown to the user.
pub fn reason_label(reason: &str) -> Option<Message> {
    Some(match reason {
        "auto" => msg("automatic", &[]),
        "bulk" => msg("before a bulk change", &[]),
        "manual" => msg("created by hand", &[]),
        "restore" => msg("before a restore", &[]),
        _ => return None,
    })
}

#[derive(Debug, PartialEq)]
pub struct SnapshotError(pub Message);

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

fn err<T>(m: Message) -> Result<T, SnapshotError> {
    Err(SnapshotError(m))
}

fn name_re() -> &'static Regex {
    static R: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| Regex::new(r"\A(?:snap-\d{8}-\d{6}(?:-\d+)?-(auto|bulk|manual|restore)\.zip)\z").unwrap())
}

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR_NAME)
}

/// (relative path with forward slashes, absolute path) of every `.rpy` file below `root`, in the order `os.walk` gives them (files first, folders sorted).
pub fn rpy_files(root: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let (mut dirs, mut files): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    dirs.push(name);
                }
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for f in files.iter().filter(|f| f.to_lowercase().ends_with(".rpy")) {
            let full = dir.join(f);
            out.push((full.strip_prefix(root).unwrap_or(&full).to_string_lossy().replace('\\', "/"), full));
        }
        for d in dirs {
            walk(&dir.join(d), root, out);
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

/// Zip the `.rpy` files. Returns the manifest (with `name`), or `None` when there is nothing to save.
pub fn create(root: &Path, data_dir: &Path, reason: &str, app_version: &str, clock: &dyn Clock) -> Result<Option<Map<String, Value>>, SnapshotError> {
    if reason_label(reason).is_none() {
        return err(msg("Unknown kind of backup.", &[]));
    }
    let files = rpy_files(root);
    if files.is_empty() {
        return Ok(None);
    }
    let total: u64 = files.iter().filter_map(|(_, f)| std::fs::metadata(f).ok()).map(|m| m.len()).sum();
    if total > MAX_BYTES {
        return err(msg(
            "The .rpy files are {size} MB – the backup is not created (limit {limit} MB).",
            &[("size", json!(total / (1024 * 1024))), ("limit", json!(MAX_BYTES / (1024 * 1024)))],
        ));
    }
    let d = dir(data_dir);
    let _ = std::fs::create_dir_all(&d);
    let now = clock.now_local();
    let base = format!("snap-{}", now.format("%Y%m%d-%H%M%S"));
    let mut name = format!("{base}-{reason}.zip");
    let mut n = 0;
    while d.join(&name).exists() {
        n += 1;
        name = format!("{base}-{n}-{reason}.zip");
    }
    let mut manifest = Map::new();
    manifest.insert("time".into(), json!(now.format("%Y-%m-%dT%H:%M:%S").to_string()));
    manifest.insert("reason".into(), json!(reason));
    manifest.insert("files".into(), json!(files.len()));
    manifest.insert("bytes".into(), json!(total));
    manifest.insert("app".into(), json!(app_version));
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).compression_level(Some(6));
    let zip_err = |e: &dyn std::fmt::Display| SnapshotError(msg("The backup cannot be written: {reason}", &[("reason", json!(e.to_string()))]));
    z.start_file("manifest.json", opts).map_err(|e| zip_err(&e))?;
    z.write_all(json_dumps(&Value::Object(manifest.clone()), false).as_bytes()).map_err(|e| zip_err(&e))?;
    for (rel, full) in &files {
        let Ok(data) = std::fs::read(full) else {
            let files_left = manifest["files"].as_i64().unwrap_or(0) - 1; // a file that vanished meanwhile is simply not saved
            manifest.insert("files".into(), json!(files_left));
            continue;
        };
        z.start_file(format!("files/{rel}"), opts).map_err(|e| zip_err(&e))?;
        z.write_all(&data).map_err(|e| zip_err(&e))?;
    }
    let bytes = z.finish().map_err(|e| zip_err(&e))?.into_inner();
    atomic_write_bytes(&d.join(&name), &bytes, None, clock)
        .map_err(|e| SnapshotError(msg("The backup cannot be written: {reason}", &[("reason", e.reason.wire())])))?;
    manifest.insert("name".into(), json!(name));
    Ok(Some(manifest))
}

fn read_manifest(path: &Path) -> Option<Map<String, Value>> {
    let mut z = ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut text = String::new();
    z.by_name("manifest.json").ok()?.read_to_string(&mut text).ok()?;
    let Value::Object(mut m) = serde_json::from_str::<Value>(&text).ok()? else { return None };
    m.insert("name".into(), json!(path.file_name()?.to_string_lossy()));
    m.insert("size".into(), json!(std::fs::metadata(path).ok()?.len()));
    let label = m.get("reason").and_then(Value::as_str).and_then(reason_label).map(|l| l.wire());
    let fallback = m.get("reason").cloned().unwrap_or(json!(""));
    m.insert("reason_label".into(), label.unwrap_or(fallback));
    Some(m)
}

/// Newest first: [{name, time, reason, files, bytes, size, ...}].
pub fn list_snapshots(data_dir: &Path) -> Vec<Map<String, Value>> {
    let d = dir(data_dir);
    let Ok(rd) = std::fs::read_dir(&d) else { return Vec::new() };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| name_re().is_match(n)).collect();
    names.sort();
    let mut out: Vec<Map<String, Value>> = names.iter().filter_map(|n| read_manifest(&d.join(n))).collect();
    out.sort_by(|a, b| b.get("time").and_then(Value::as_str).unwrap_or("").cmp(a.get("time").and_then(Value::as_str).unwrap_or("")));
    out
}

pub fn last_age_hours(data_dir: &Path, clock: &dyn Clock) -> Option<f64> {
    let snaps = list_snapshots(data_dir);
    let t = snaps.first()?.get("time")?.as_str()?;
    let then = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some((clock.now_local() - then).num_milliseconds() as f64 / 1000.0 / 3600.0)
}

/// Delete the oldest snapshots beyond `keep` (manual ones are kept the longest). Returns how many were removed.
pub fn prune(data_dir: &Path, keep: i64) -> usize {
    let snaps = list_snapshots(data_dir);
    let extra = snaps.len() as i64 - keep.max(1);
    if extra <= 0 {
        return 0;
    }
    let mut order: Vec<&Map<String, Value>> = snaps.iter().collect();
    order.sort_by_key(|m| (m.get("reason").and_then(Value::as_str) == Some("manual"), m.get("time").and_then(Value::as_str).unwrap_or("").to_string()));
    order.iter().take(extra as usize).filter(|m| std::fs::remove_file(dir(data_dir).join(m["name"].as_str().unwrap_or(""))).is_ok()).count()
}

fn snapshot_path(data_dir: &Path, name: &str) -> Result<PathBuf, SnapshotError> {
    if !name_re().is_match(name) {
        return err(msg("Invalid backup name.", &[]));
    }
    let p = dir(data_dir).join(name);
    if !p.is_file() {
        return err(msg("The backup does not exist.", &[]));
    }
    Ok(p)
}

pub fn delete(data_dir: &Path, name: &str) -> Result<(), SnapshotError> {
    let p = snapshot_path(data_dir, name)?;
    std::fs::remove_file(p).map_err(|e| SnapshotError(msg("The backup cannot be deleted: {reason}", &[("reason", json!(e.to_string()))])))
}

/// The files of a snapshot: (name inside the zip without `files/`, bytes), in the order of the zip.
fn entries(path: &Path) -> Result<Vec<(String, Vec<u8>)>, SnapshotError> {
    let bad = |e: &dyn std::fmt::Display| SnapshotError(msg("The backup cannot be read: {reason}", &[("reason", json!(e.to_string()))]));
    let mut z = ZipArchive::new(std::fs::File::open(path).map_err(|e| bad(&e))?).map_err(|e| bad(&e))?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| bad(&e))?;
        let name = f.name().to_string();
        let Some(rel) = name.strip_prefix("files/") else { continue };
        if name.ends_with('/') {
            continue;
        }
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| bad(&e))?;
        out.push((rel.to_string(), data));
    }
    Ok(out)
}

/// For every file in the snapshot: same | changed | missing (not in the folder any more), against the current state.
pub fn compare(root: &Path, data_dir: &Path, name: &str) -> Result<Vec<(String, &'static str)>, SnapshotError> {
    let p = snapshot_path(data_dir, name)?;
    let mut out: Vec<(String, &'static str)> = Vec::new();
    for (rel, data) in entries(&p)? {
        let cur = root.join(&rel);
        let state = if !cur.is_file() {
            "missing"
        } else {
            match std::fs::read(&cur) {
                Ok(b) if sha1_hex(&b) == sha1_hex(&data) => "same",
                _ => "changed",
            }
        };
        out.push((rel, state));
    }
    let rank = |s: &str| match s {
        "changed" => 0,
        "missing" => 1,
        _ => 2,
    };
    out.sort_by_key(|(f, s)| (rank(s), f.to_lowercase()));
    Ok(out)
}

/// `root/rel` with `.` and `..` resolved, without needing the file to exist (`Path.resolve()` of Python).
fn resolve(root: &Path, rel: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out
}

/// What a restore did: the files put back and the safety snapshot made before it.
pub type Restored = (Vec<String>, Option<Map<String, Value>>);

/// Put files of a snapshot back (all, or the chosen ones). First saves the current state as a "restore" snapshot. Returns (restored files, the safety snapshot).
pub fn restore(root: &Path, data_dir: &Path, name: &str, files: Option<&[String]>, app_version: &str, clock: &dyn Clock) -> Result<Restored, SnapshotError> {
    let p = snapshot_path(data_dir, name)?;
    let root = std::fs::canonicalize(root).map(|r| PathBuf::from(r.to_string_lossy().trim_start_matches(r"\\?\"))).unwrap_or_else(|_| root.to_path_buf());
    let wanted: Option<BTreeSet<String>> = files.map(|f| f.iter().map(|s| s.replace('\\', "/")).collect());
    let safety = create(&root, data_dir, "restore", app_version, clock)?;
    let mut restored = Vec::new();
    for (rel, data) in entries(&p)? {
        if wanted.as_ref().is_some_and(|w| !w.contains(&rel)) {
            continue;
        }
        let target = resolve(&root, &rel);
        if target != root && !target.starts_with(&root) {
            return err(msg("The backup contains a path outside the project folder, the restore was cancelled.", &[]));
        }
        atomic_write_bytes(&target, &data, None, clock)
            .map_err(|e| SnapshotError(msg("Cannot write {rel}: {reason}", &[("rel", json!(rel)), ("reason", e.reason.wire())])))?;
        restored.push(rel);
    }
    Ok((restored, safety))
}

/// True when no snapshot exists or the newest one is older than the interval.
pub fn due(data_dir: &Path, interval_hours: f64, clock: &dyn Clock) -> bool {
    last_age_hours(data_dir, clock).is_none_or(|age| age >= interval_hours)
}

pub fn recent(data_dir: &Path, minutes: f64, clock: &dyn Clock) -> bool {
    last_age_hours(data_dir, clock).is_some_and(|age| age * 60.0 < minutes)
}

/// The files of a snapshot with their SHA-1, sorted (for tests and for a listing without unpacking).
pub fn digests(data_dir: &Path, name: &str) -> Result<Vec<(String, String)>, SnapshotError> {
    let path = dir(data_dir).join(name);
    let bad = |e: &dyn std::fmt::Display| SnapshotError(msg("The backup cannot be read: {reason}", &[("reason", json!(e.to_string()))]));
    let mut z = ZipArchive::new(std::fs::File::open(&path).map_err(|e| bad(&e))?).map_err(|e| bad(&e))?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| bad(&e))?;
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| bad(&e))?;
        out.push((f.name().to_string(), sha1_hex(&data)));
    }
    out.sort();
    Ok(out)
}
