//! Port of `reference/python/core/lexo/project.py`: a translation project folder: discovery, incremental refresh, per-file counts.
//!
//! The store is not held by the project (the Python object keeps a reference to it): the functions that need it take it as a parameter.
//! File names (`rel`) are written with the separator of the system, like Python does.
use crate::clock::Clock;
use crate::lang::Pack;
use crate::messages::{msg, Message};
use crate::rpy::{self, Entry, ParsedFile};
use crate::store::{parse_corr, parse_value, text_hash, Kind, Label, Store};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Shared project data (datadir.rs).
pub const DATA_DIR_NAME: &str = ".aside";
pub const IGNORED_DIRS: [&str; 6] = [DATA_DIR_NAME, ".lexo", "_progress_data", ".git", "__pycache__", "node_modules"];

/// One row per file: name, translated (including the manual override), remaining, total, proofread, the automatic translated count.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub done: i64,
    pub todo: i64,
    pub total: i64,
    pub corr: i64,
    pub auto: i64,
}

pub const STATUS_ORDER: [&str; 5] = ["none", "wip", "tr", "rev", "done"];

pub fn file_status(done: i64, todo: i64, total: i64, corr: i64) -> &'static str {
    if corr >= total {
        "done"
    } else if todo == 0 {
        if corr > 0 {
            "rev"
        } else {
            "tr"
        }
    } else if done > 0 {
        "wip"
    } else {
        "none"
    }
}

/// The project folder is missing or cannot be read.
#[derive(Debug, PartialEq)]
pub struct ProjectUnavailable(pub Message);

/// What a file looked like when it was read: size and time.
pub type Sig = (u64, i128);

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub rel: String,
    pub sig: Sig,
    pub parsed: ParsedFile,
}

#[derive(Clone, Debug, Default)]
pub struct RefreshReport {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
    /// (old, new)
    pub renamed: Vec<(String, String)>,
    pub unchanged: usize,
    pub errors: Vec<Message>,
    /// No translatable lines.
    pub empty: Vec<String>,
    /// rel -> the file before the change (for the text history)
    pub prev: IndexMap<String, FileInfo>,
    pub seconds: f64,
}

impl RefreshReport {
    pub fn changed(&self) -> bool {
        !(self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty() && self.renamed.is_empty())
    }

    pub fn summary(&self) -> Value {
        if !self.changed() && self.errors.is_empty() {
            return msg("No changes.", &[]).wire();
        }
        let mut parts: Vec<Value> = Vec::new();
        let mut add = |n: usize, t: &str| {
            if n > 0 {
                parts.push(msg(t, &[("n", json!(n))]).wire());
            }
        };
        add(self.added.len(), "added {n}");
        add(self.modified.len(), "changed {n}");
        add(self.renamed.len(), "renamed {n}");
        add(self.removed.len(), "removed {n}");
        add(self.errors.len(), "problems {n}");
        json!({"$j": parts, "sep": ", ", "end": "."})
    }

    pub fn to_json(&self) -> Value {
        json!({
            "added": self.added, "removed": self.removed, "modified": self.modified,
            "renamed": self.renamed.iter().map(|(a, b)| json!({"from": a, "to": b})).collect::<Vec<_>>(),
            "unchanged": self.unchanged, "errors": self.errors.iter().map(Message::wire).collect::<Vec<_>>(), "empty": self.empty,
            "seconds": crate::pyfmt::round_digits(self.seconds, 3), "changed": self.changed(), "summary": self.summary(),
        })
    }
}

fn is_rpy(name: &str) -> bool {
    name.to_lowercase().ends_with(".rpy")
}

/// (ok, message). A usable project folder exists, is readable and contains `.rpy` files.
pub fn check_project_dir(path: &str) -> (bool, Option<Message>) {
    let shown = |p: &str| json!(p);
    if crate::util::py_strip(path).is_empty() {
        return (false, Some(msg("No folder is selected.", &[])));
    }
    let p = Path::new(path);
    if !p.exists() {
        return (false, Some(msg("The folder does not exist: {p}", &[("p", shown(path))])));
    }
    if !p.is_dir() {
        return (false, Some(msg("It is not a folder: {p}", &[("p", shown(path))])));
    }
    if std::fs::read_dir(p).is_err() {
        return (false, Some(msg("I have no permission to read the folder: {p}", &[("p", shown(path))])));
    }
    fn has_rpy(dir: &Path) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else { return false };
        let mut dirs = Vec::new();
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                if !IGNORED_DIRS.contains(&name.as_str()) {
                    dirs.push(e.path());
                }
            } else if is_rpy(&name) {
                return true;
            }
        }
        dirs.iter().any(|d| has_rpy(d))
    }
    if has_rpy(p) {
        (true, None)
    } else {
        (false, Some(msg("There are no .rpy files in the folder: {p}", &[("p", shown(path))])))
    }
}

/// When `path` is a game folder (or its `game` folder, or `tl`) instead of one language folder, the language folder inside it. Returns (folder, note).
/// Several languages: the error names them (the user must choose).
pub fn find_translation_folder(path: &Path) -> Result<(PathBuf, Option<Message>), Message> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let candidates = [if name.to_lowercase() == "tl" { Some(path.to_path_buf()) } else { None }, Some(path.join("tl")), Some(path.join("game").join("tl"))];
    for tl in candidates.into_iter().flatten() {
        if !tl.is_dir() {
            continue;
        }
        let mut langs: Vec<PathBuf> = std::fs::read_dir(&tl)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|d| d.is_dir() && !IGNORED_DIRS.contains(&d.file_name().unwrap().to_string_lossy().as_ref()) && !rpy_files_below(d).is_empty())
                    .collect()
            })
            .unwrap_or_default();
        langs.sort();
        if langs.len() == 1 {
            let chosen = if name.is_empty() { path.to_string_lossy().into_owned() } else { name };
            let folder = langs[0].file_name().unwrap().to_string_lossy().into_owned();
            let note =
                msg("The folder {chosen} is a game folder; its translation {folder} was opened.", &[("chosen", json!(chosen)), ("folder", json!(folder))]);
            return Ok((langs.remove(0), Some(note)));
        }
        if langs.len() > 1 {
            let names: Vec<String> = langs.iter().map(|d| d.file_name().unwrap().to_string_lossy().into_owned()).collect();
            return Err(msg("The folder holds several translations ({names}); choose one of them.", &[("names", json!(names.join(", ")))]));
        }
    }
    Ok((path.to_path_buf(), None))
}

fn rpy_files_below(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                out.extend(rpy_files_below(&p));
            } else if is_rpy(&e.file_name().to_string_lossy()) {
                out.push(p);
            }
        }
    }
    out
}

pub struct Project {
    pub root: PathBuf,
    skip: BTreeSet<String>,
    pub symbols_as_done: Option<bool>,
    pack: Pack,
    /// rel -> file (only files with translatable lines)
    pub files: IndexMap<String, FileInfo>,
    /// rel -> signature for every file seen, also empty ones
    known: IndexMap<String, Sig>,
    pub loaded_at: Option<f64>,
    /// What the last refresh found (also kept for the overview).
    pub last_report: Option<RefreshReport>,
}

type Found = IndexMap<String, (PathBuf, Sig)>;

impl Project {
    pub fn new(root: &Path, skip: &[String], symbols_as_done: Option<bool>, pack: &Pack) -> Project {
        Project {
            root: root.to_path_buf(),
            skip: skip.iter().map(|s| s.to_lowercase()).collect(),
            symbols_as_done,
            pack: pack.clone(),
            files: IndexMap::new(),
            known: IndexMap::new(),
            loaded_at: None,
            last_report: None,
        }
    }

    /// The file names (lower case) that are left out of the project.
    pub fn skip(&self) -> &BTreeSet<String> {
        &self.skip
    }

    pub fn set_skip(&mut self, names: &[String]) {
        self.skip = names.iter().map(|s| s.to_lowercase()).collect();
    }

    // ------------------------------------------------------------ discovery
    fn discover(&self, errors: &mut Vec<Message>, failed: &mut Vec<PathBuf>) -> Result<Found, ProjectUnavailable> {
        if !self.root.is_dir() {
            return Err(ProjectUnavailable(msg("The project folder is not available: {root}", &[("root", json!(self.root.to_string_lossy()))])));
        }
        let mut found = Found::new();
        self.walk(&self.root, &mut found, errors, failed);
        Ok(found)
    }

    fn walk(&self, dir: &Path, found: &mut Found, errors: &mut Vec<Message>, failed: &mut Vec<PathBuf>) {
        let rd = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                failed.push(dir.to_path_buf());
                errors
                    .push(msg("The folder cannot be browsed: {name} ({reason})", &[("name", json!(dir.to_string_lossy())), ("reason", json!(e.to_string()))]));
                return;
            }
        };
        let (mut dirs, mut files): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                if !IGNORED_DIRS.contains(&name.as_str()) {
                    dirs.push(name);
                }
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for fn_ in files {
            if !is_rpy(&fn_) || self.skip.contains(&fn_.to_lowercase()) {
                continue;
            }
            let full = dir.join(&fn_);
            match std::fs::metadata(&full) {
                Ok(m) => {
                    let ns = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i128);
                    let rel = full.strip_prefix(&self.root).unwrap_or(&full).to_string_lossy().into_owned();
                    found.insert(rel, (full, (m.len(), ns)));
                }
                Err(e) => {
                    failed.push(full.clone());
                    let rel = full.strip_prefix(&self.root).unwrap_or(&full).to_string_lossy().into_owned();
                    errors.push(Message::new(&format!("{rel}: nelze zjistit stav ({e})"), None, Default::default()));
                }
            }
        }
        for d in dirs {
            self.walk(&dir.join(d), found, errors, failed);
        }
    }

    // ------------------------------------------------------------ refresh
    /// Rescan the folder. Only new or changed files are parsed again. Reports added / removed / modified / renamed files. Metadata of removed files stays in the store, so a
    /// file that comes back (or a temporarily unavailable network drive) loses nothing. A detected rename is returned in `report.renamed`; the caller moves the metadata
    /// (see `migrate_metadata`).
    pub fn refresh(&mut self, force: bool, clock: &dyn Clock) -> Result<RefreshReport, ProjectUnavailable> {
        let t0 = std::time::Instant::now();
        let mut report = RefreshReport::default();
        let mut failed: Vec<PathBuf> = Vec::new();
        let found = self.discover(&mut report.errors, &mut failed)?;
        let mut fresh: IndexMap<String, FileInfo> = IndexMap::new();
        for (rel, (full, sig)) in &found {
            if self.known.get(rel) == Some(sig) && !force {
                continue;
            }
            match std::fs::read(full) {
                Ok(bytes) => {
                    fresh.insert(rel.clone(), FileInfo { rel: rel.clone(), sig: *sig, parsed: rpy::parse_bytes(&bytes, &self.pack) });
                }
                Err(e) => report.errors.push(msg(
                    "{rel}: the file cannot be read ({reason}); the last known state is kept.",
                    &[("rel", json!(rel)), ("reason", json!(e.to_string()))],
                )),
            }
        }
        let unreachable = |rel: &str| {
            let p = self.root.join(rel);
            failed.iter().any(|f| p == *f || p.starts_with(f))
        };
        let previously: BTreeSet<String> = self.known.keys().cloned().collect();
        let mut gone: BTreeSet<String> = previously.iter().filter(|r| !found.contains_key(*r) && !unreachable(r)).cloned().collect();
        let mut added: Vec<String> = fresh.keys().filter(|r| !previously.contains(*r)).cloned().collect();
        let modified: Vec<String> = fresh.keys().filter(|r| previously.contains(*r)).cloned().collect();

        let mut renames: Vec<(String, String)> = Vec::new();
        for new in added.clone() {
            let nf = &fresh[&new].parsed;
            let mut best: Option<String> = None;
            for old in &gone {
                let Some(of) = self.files.get(old) else { continue };
                if !of.parsed.digest.is_empty() && of.parsed.digest == nf.digest {
                    best = Some(old.clone());
                    break;
                }
                let ids_old: BTreeSet<&String> = of.parsed.block_ids.iter().map(|b| &b.1).collect();
                let ids_new: BTreeSet<&String> = nf.block_ids.iter().map(|b| &b.1).collect();
                let name_of = |p: &str| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned());
                if !ids_old.is_empty() && !ids_new.is_empty() && name_of(old) == name_of(&new) {
                    let (inter, union) = (ids_old.intersection(&ids_new).count(), ids_old.union(&ids_new).count());
                    if inter as f64 / union as f64 >= 0.8 {
                        best = Some(old.clone());
                        break;
                    }
                }
            }
            if let Some(old) = best {
                gone.remove(&old);
                added.retain(|a| *a != new);
                renames.push((old, new));
            }
        }

        for rel in fresh.keys() {
            if let Some(f) = self.files.get(rel) {
                report.prev.insert(rel.clone(), f.clone());
            }
        }
        for (rel, fi) in fresh {
            self.known.insert(rel.clone(), fi.sig);
            if fi.parsed.entries.is_empty() {
                self.files.shift_remove(&rel);
                report.empty.push(rel);
            } else {
                self.files.insert(rel, fi);
            }
        }
        for (old, _) in &renames {
            self.known.shift_remove(old);
            self.files.shift_remove(old);
        }
        for rel in &gone {
            self.known.shift_remove(rel);
            self.files.shift_remove(rel);
        }

        report.added = sorted(added.iter().filter(|r| self.files.contains_key(*r)).cloned().collect());
        report.modified = sorted(modified.iter().filter(|r| self.files.contains_key(*r)).cloned().collect());
        report.removed = gone.into_iter().collect();
        renames.sort();
        report.renamed = renames;
        report.unchanged = self.files.len().saturating_sub(report.added.len() + report.modified.len() + report.renamed.len());
        report.seconds = t0.elapsed().as_secs_f64();
        self.loaded_at = Some(clock.epoch());
        self.last_report = Some(report.clone());
        Ok(report)
    }

    /// Move proofread counts / assignments / approvals of renamed files to their new names. One undoable step. Returns the number of files whose metadata moved.
    pub fn migrate_metadata(store: &mut Store, renames: &[(String, String)], clock: &dyn Clock) -> usize {
        let moved: Vec<(String, String)> = renames
            .iter()
            .filter(|(o, _)| store.corr.contains_key(o) || store.assign.contains_key(o) || store.approved.contains_key(o) || store.proof.contains_key(o))
            .cloned()
            .collect();
        if moved.is_empty() {
            return 0;
        }
        let kinds = [Kind::Corr, Kind::Assign, Kind::Approved, Kind::Proof];
        let list = moved.clone();
        let _ = store.change(
            Label::Msg(msg("Renamed files", &[])),
            &kinds,
            move |st| {
                for (old, new) in &list {
                    if let Some(v) = st.corr.get(old).cloned() {
                        st.corr.entry(new.clone()).or_insert(v);
                    }
                    if let Some(v) = st.assign.shift_remove(old) {
                        st.assign.entry(new.clone()).or_insert(v);
                    }
                    if let Some(v) = st.approved.shift_remove(old) {
                        st.approved.entry(new.clone()).or_insert(v);
                    }
                    if let Some(v) = st.proof.shift_remove(old) {
                        st.proof.entry(new.clone()).or_insert(v);
                    }
                }
            },
            clock,
        );
        store.clear_undo(); // older undo steps refer to the old names
        moved.len()
    }

    // ------------------------------------------------------------ counts
    /// (translated, remaining, approved) for a file: approved pending lines count as translated.
    pub fn counts(&self, store: &Store, rel: &str) -> (usize, usize, usize) {
        let empty = BTreeSet::new();
        rpy::count(&self.files[rel].parsed.entries, store.approved.get(rel).unwrap_or(&empty), self.symbols_as_done)
    }

    /// Translated, or an untranslated-looking line the translator approved.
    pub fn is_translated(&self, store: &Store, rel: &str, e: &Entry) -> bool {
        !e.pending() || store.approved.get(rel).is_some_and(|s| s.contains(&e.key()))
    }

    /// `""` (not proofread) | `ok` | `fix` (flagged) | `stale` (was ok, but the translation changed since).
    pub fn proof_state(&self, store: &Store, rel: &str, e: &Entry) -> &'static str {
        let Some(m) = store.proof.get(rel).and_then(|t| t.get(&e.key())).filter(|m| !m.is_empty()) else { return "" };
        if m.get("s").and_then(Value::as_str) == Some("fix") {
            return "fix";
        }
        if m.get("h").and_then(Value::as_str) == Some(text_hash(&e.target).as_str()) {
            "ok"
        } else {
            "stale"
        }
    }

    /// (lines marked proofread, lines flagged "to fix"): only valid marks on translated lines.
    pub fn proof_counts(&self, store: &Store, rel: &str) -> (i64, i64) {
        let Some(marks) = store.proof.get(rel).filter(|m| !m.is_empty()) else { return (0, 0) };
        let (mut ok, mut fix) = (0, 0);
        for e in &self.files[rel].parsed.entries {
            if marks.contains_key(&e.key()) && self.is_translated(store, rel, e) {
                match self.proof_state(store, rel, e) {
                    "ok" => ok += 1,
                    "fix" => fix += 1,
                    _ => {}
                }
            }
        }
        (ok, fix)
    }

    /// Lines that were marked proofread but whose translation changed since (they need a new look).
    pub fn proof_stale(&self, store: &Store, rel: &str) -> i64 {
        let Some(marks) = store.proof.get(rel).filter(|m| !m.is_empty()) else { return 0 };
        self.files[rel]
            .parsed
            .entries
            .iter()
            .filter(|e| marks.contains_key(&e.key()) && self.is_translated(store, rel, e) && self.proof_state(store, rel, e) == "stale")
            .count() as i64
    }

    /// One Row per file. Proofread = the hand-typed baseline (proofread.csv) + lines marked one by one.
    pub fn rows(&self, store: &Store) -> Vec<Row> {
        let mut names: Vec<&String> = self.files.keys().collect();
        names.sort_by_key(|r| r.to_lowercase());
        names
            .into_iter()
            .map(|rel| {
                let (done, todo, _appr) = self.counts(store, rel);
                let (done, todo) = (done as i64, todo as i64);
                let total = done + todo;
                let raw = store.corr.get(rel).cloned().unwrap_or_default();
                let shown = parse_value(&raw.t, total).unwrap_or(done);
                let corr = total.min(parse_corr(&raw.c, total) + self.proof_counts(store, rel).0);
                Row { name: rel.clone(), done: shown, todo: total - shown, total, corr, auto: done }
            })
            .collect()
    }

    pub fn row(&self, store: &Store, rel: &str) -> Option<Row> {
        self.rows(store).into_iter().find(|r| r.name == rel)
    }

    pub fn entries(&self, rel: &str) -> &[Entry] {
        &self.files[rel].parsed.entries
    }

    /// Absolute path of a project file; fails for anything outside the project.
    pub fn path_of(&self, rel: &str) -> Result<PathBuf, Message> {
        let full = resolve_lexical(&self.root.join(rel));
        let root = std::fs::canonicalize(&self.root)
            .map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\")))
            .unwrap_or_else(|_| resolve_lexical(&self.root));
        let canonical_full = std::fs::canonicalize(&full).map(|p| PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\"))).unwrap_or_else(|_| {
            // the file may not exist: resolve the folder it lies in
            match (full.parent().and_then(|p| std::fs::canonicalize(p).ok()), full.file_name()) {
                (Some(p), Some(n)) => PathBuf::from(p.to_string_lossy().trim_start_matches(r"\\?\")).join(n),
                _ => full.clone(),
            }
        });
        if canonical_full != root && !canonical_full.starts_with(&root) {
            return Err(msg("The file is not inside the project folder.", &[]));
        }
        Ok(canonical_full)
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// `.` and `..` resolved without touching the disk.
fn resolve_lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// (translated, total, proofread, percent translated, percent proofread) of all rows.
pub fn totals_of(rows: &[Row]) -> (i64, i64, i64, f64, f64) {
    let s_done: i64 = rows.iter().map(|r| r.done).sum();
    let s_total: i64 = rows.iter().map(|r| r.total).sum();
    let s_corr: i64 = rows.iter().map(|r| r.corr).sum();
    let pct = if s_total != 0 { s_done as f64 / s_total as f64 * 100.0 } else { 0.0 };
    let cpct = if s_total != 0 { s_corr as f64 / s_total as f64 * 100.0 } else { 0.0 };
    (s_done, s_total, s_corr, pct, cpct)
}
