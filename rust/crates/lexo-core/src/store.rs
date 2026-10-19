//! Port of `reference/python/core/lexo/store.py`: translator-maintained metadata, stored in small CSV files in the data folder (Excel-friendly).
//!
//! proofread.csv, assignments.csv, approved.csv, episode_states.csv, line_marks.csv, ignored.csv, glossary.csv (history.csv: see `history`).
//! Every write is atomic and backed up (`<local data dir>/backups`). Mutations go through `change()`, which keeps an undo stack of the most recent changes.
//! The layout of the open project (valid group numbers, the word for a group) is a field of the store here, a process-wide global in the Python module.
use crate::clock::Clock;
use crate::events::{EventLog, Key, K};
use crate::fsutil::{read_rows, write_csv, WriteFailed};
use crate::layout::Layout;
use crate::messages::{msg, Message};
use crate::util::{fold, norm_ws, sha1_hex};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const EP_STATE_KEYS: [&str; 5] = ["none", "wip", "tr", "proof", "done"];
const YES_WORDS: [&str; 6] = ["yes", "y", "x", "done", "ok", "1x"];
pub const UNDO_LIMIT: usize = 30;
pub const GLOSSARY_TYPES: [&str; 2] = ["speaker", "term"];
/// The kinds of data that have an event log.
pub const EVENT_KINDS: [&str; 6] = ["corr", "assign", "approved", "episodes", "proof", "ignored"];

/// `[(key, label)]` of the episode states; the labels are messages.
pub fn ep_states() -> Vec<(&'static str, Message)> {
    let labels = [msg("Not started", &[]), msg("In progress", &[]), msg("Translated", &[]), msg("In proofreading", &[]), msg("Done", &[])];
    EP_STATE_KEYS.iter().copied().zip(labels).collect()
}

/// Short fingerprint of a translation; a proofread mark is only valid while it still matches.
pub fn text_hash(text: &str) -> String {
    sha1_hex(text.as_bytes())[..12].to_string()
}

/// Number / percent / yes-word -> number of lines. Empty or invalid -> `None`.
pub fn parse_value(value: &str, total: i64) -> Option<i64> {
    let v = crate::util::py_strip(value).to_lowercase().replace(',', ".");
    if v.is_empty() {
        return None;
    }
    if YES_WORDS.contains(&v.as_str()) {
        return Some(total);
    }
    if let Some(p) = v.strip_suffix('%') {
        let f: f64 = p.trim().parse().ok().filter(|f: &f64| f.is_finite())?;
        return Some((crate::pyfmt::round_half_even(f * total as f64 / 100.0) as i64).clamp(0, total.max(0)));
    }
    let f: f64 = v.parse().ok().filter(|f: &f64| f.is_finite())?;
    Some((f.trunc() as i64).clamp(0, total.max(0)))
}

pub fn parse_corr(value: &str, total: i64) -> i64 {
    parse_value(value, total).unwrap_or(0)
}

fn digits_after(v: &str, prefix: &str) -> Option<String> {
    let rest = v.strip_prefix(prefix)?;
    (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())).then(|| rest.to_string())
}

pub fn valid_assign(v: &str, layout: &Layout) -> bool {
    if v.is_empty() || v == "gen" {
        return true;
    }
    digits_after(v, "ep:").and_then(|d| d.parse::<i64>().ok()).is_some_and(|n| layout.active_groups().contains(&n))
}

pub fn assign_to_text(v: &str) -> String {
    if v == "gen" {
        "general".into()
    } else if let Some(rest) = v.strip_prefix("ep:") {
        format!("episode {rest}")
    } else {
        String::new()
    }
}

pub fn assign_from_text(t: &str) -> String {
    let f = fold(t);
    if f == "general" || f == "gen" {
        return "gen".into();
    }
    let digits: String = f.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse::<i64>().map_or(String::new(), |n| format!("ep:{n}"))
}

/// A text that may be a message (words that have a translation) or plain (a name made by the user).
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Plain(String),
    Msg(Message),
}

impl Label {
    pub fn text(&self) -> String {
        match self {
            Label::Plain(s) => s.clone(),
            Label::Msg(m) => m.text(),
        }
    }
    pub fn wire(&self) -> Value {
        match self {
            Label::Plain(s) => json!(s),
            Label::Msg(m) => m.wire(),
        }
    }
}

/// "Episode 3", "Chapter 3" ...: the word is the `group` label of the project; a word without a translation is used as it is.
pub fn group_label(n: &str, layout: &Layout) -> Label {
    if let Ok(i) = n.parse::<usize>() {
        if !layout.folders.is_empty() && (1..=layout.folders.len()).contains(&i) {
            return Label::Plain(layout.folders[i - 1].rsplit('/').next().unwrap_or("").to_string());
            // one group per folder: the group is called like the folder
        }
    }
    let word = layout.labels.get("group").map_or("Episode", String::as_str);
    let make = |t: &str| Label::Msg(msg(t, &[("n", json!(n))]));
    match word {
        "Episode" => make("Episode {n}"),
        "Chapter" => make("Chapter {n}"),
        "Part" => make("Part {n}"),
        "Day" => make("Day {n}"),
        "Update" => make("Update {n}"),
        "Act" => make("Act {n}"),
        other => Label::Plain(format!("{other} {n}")),
    }
}

pub fn assign_label(v: &str, layout: &Layout) -> Label {
    match v {
        "" => Label::Msg(msg("Unassigned", &[])),
        "gen" => Label::Msg(msg("General (the whole game)", &[])),
        _ => group_label(v.get(3..).unwrap_or(""), layout),
    }
}

/// Episode number of an `ep:N` assignment, else `None`.
pub fn ep_of(v: &str) -> Option<i64> {
    digits_after(v, "ep:").and_then(|d| d.parse().ok())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Corr,
    Assign,
    Approved,
    Episodes,
    Proof,
    Glossary,
    Ignored,
}

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Corr, Kind::Assign, Kind::Approved, Kind::Episodes, Kind::Proof, Kind::Glossary, Kind::Ignored];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Corr => "corr",
            Kind::Assign => "assign",
            Kind::Approved => "approved",
            Kind::Episodes => "episodes",
            Kind::Proof => "proof",
            Kind::Glossary => "glossary",
            Kind::Ignored => "ignored",
        }
    }

    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.name() == name)
    }

    fn file(self) -> &'static str {
        match self {
            Kind::Corr => "proofread.csv",
            Kind::Assign => "assignments.csv",
            Kind::Approved => "approved.csv",
            Kind::Episodes => "episode_states.csv",
            Kind::Proof => "line_marks.csv",
            Kind::Glossary => "glossary.csv",
            Kind::Ignored => "ignored.csv",
        }
    }
}

/// A manual correction of one file: the proofread lines and the manual translated override.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Corr {
    pub c: String,
    pub t: String,
}

/// A glossary entry: the full name, `speaker` or `term`, a note.
#[derive(Clone, Debug, PartialEq)]
pub struct Gloss {
    pub n: String,
    pub t: String,
    pub c: String,
}

/// A mark (proofread, ignored finding): a JSON object with `s`, `h`, `n`, `d` (as read from the file or from an event).
pub type Mark = Map<String, Value>;

type Marks2 = IndexMap<String, IndexMap<(String, String), Mark>>;
type Marks3 = IndexMap<String, IndexMap<(String, String, String), Mark>>;

/// A copy of the data of one kind (for the undo steps).
#[derive(Clone, Debug, PartialEq)]
enum Snapshot {
    Corr(IndexMap<String, Corr>),
    Assign(IndexMap<String, String>),
    Approved(IndexMap<String, BTreeSet<(String, String)>>),
    Episodes(IndexMap<i64, String>),
    Proof(Marks2),
    Glossary(IndexMap<String, Gloss>),
    Ignored(Marks3),
}

#[derive(Clone, Debug)]
struct UndoStep {
    label: Label,
    kinds: Vec<Kind>,
    before: Vec<Snapshot>,
}

pub struct Store {
    pub dir: PathBuf,
    pub backups: PathBuf,
    pub layout: Layout,
    /// name -> manual correction
    pub corr: IndexMap<String, Corr>,
    /// name -> `""` | `gen` | `ep:N`
    pub assign: IndexMap<String, String>,
    /// name -> {(block, normalised source)}
    pub approved: IndexMap<String, BTreeSet<(String, String)>>,
    /// episode number -> state key
    pub episodes: IndexMap<i64, String>,
    /// name -> {(block, normalised source): mark}
    pub proof: Marks2,
    /// code -> entry
    pub glossary: IndexMap<String, Gloss>,
    /// name -> {(block, normalised source, issue code): mark}
    pub ignored: Marks3,
    undo_stack: Vec<UndoStep>,
    /// Kinds whose last save failed.
    pub unsaved: BTreeSet<&'static str>,
    pub warnings: Vec<Message>,
    /// The event log of the open project (`attach_events`).
    pub events: Option<EventLog>,
}

/// The names sorted by their lower-case form; stable, so names that differ only in case keep the order they were added in.
fn lower_sorted(mut keys: Vec<&String>) -> Vec<&String> {
    keys.sort_by_key(|k| k.to_lowercase());
    keys
}

fn text_of(row: &[String], i: usize) -> String {
    row.get(i).cloned().unwrap_or_default()
}

fn plain(text: String) -> Message {
    Message::new(&text, None, Map::new())
}

impl Store {
    pub fn new(data_dir: &Path, local_dir: Option<&Path>, layout: Layout) -> Store {
        Store {
            dir: data_dir.to_path_buf(),
            backups: local_dir.unwrap_or(data_dir).join("backups"),
            layout,
            corr: IndexMap::new(),
            assign: IndexMap::new(),
            approved: IndexMap::new(),
            episodes: IndexMap::new(),
            proof: IndexMap::new(),
            glossary: IndexMap::new(),
            ignored: IndexMap::new(),
            undo_stack: Vec::new(),
            unsaved: BTreeSet::new(),
            warnings: Vec::new(),
            events: None,
        }
    }

    pub fn path(&self, kind: Kind) -> PathBuf {
        self.dir.join(kind.file())
    }

    pub fn undo_labels(&self) -> Vec<String> {
        self.undo_stack.iter().map(|s| s.label.text()).collect()
    }

    /// The labels of the undo steps for an answer (messages stay messages).
    pub fn undo_label_values(&self) -> Vec<Value> {
        self.undo_stack.iter().map(|s| s.label.wire()).collect()
    }

    // ------------------------------------------------------------ load
    pub fn load(&mut self) -> &mut Store {
        self.warnings.clear();
        for kind in Kind::ALL {
            match read_rows(&self.path(kind)) {
                Ok(rows) => self.load_kind(kind, &rows),
                Err(e) => {
                    let m = msg("{file} cannot be loaded: {reason}", &[("file", json!(kind.file())), ("reason", json!(e.to_string()))]);
                    self.warnings.push(m);
                }
            }
        }
        self
    }

    fn load_kind(&mut self, kind: Kind, rows: &[Vec<String>]) {
        let body = rows.get(1..).unwrap_or(&[]);
        match kind {
            Kind::Corr => {
                self.corr = body.iter().map(|r| (r[0].clone(), Corr { c: text_of(r, 1), t: text_of(r, 2) })).collect();
            }
            Kind::Assign => {
                self.assign = IndexMap::new();
                for r in body.iter().filter(|r| r.len() >= 2) {
                    let v = assign_from_text(&r[1]);
                    if !v.is_empty() && valid_assign(&v, &self.layout) {
                        self.assign.insert(r[0].clone(), v);
                    } else if !v.is_empty() {
                        self.warnings
                            .push(msg("assignments.csv: {a} → {b} is not a valid assignment, ignoring it.", &[("a", json!(r[0])), ("b", json!(r[1]))]));
                    }
                }
            }
            Kind::Approved => {
                self.approved = IndexMap::new();
                for r in body.iter().filter(|r| r.len() >= 3 && !r[0].is_empty()) {
                    self.approved.entry(r[0].clone()).or_default().insert((r[1].clone(), norm_ws(&r[2])));
                }
            }
            Kind::Glossary => {
                self.glossary = IndexMap::new();
                for r in body.iter().filter(|r| !r.is_empty() && !crate::util::py_strip(&r[0]).is_empty()) {
                    let t = if r.len() > 2 && GLOSSARY_TYPES.contains(&r[2].as_str()) { r[2].clone() } else { "speaker".into() };
                    let n = if r.len() > 1 { crate::util::py_strip(&r[1]).to_string() } else { String::new() };
                    self.glossary.insert(crate::util::py_strip(&r[0]).to_string(), Gloss { n, t, c: text_of(r, 3) });
                }
            }
            Kind::Proof => {
                self.proof = IndexMap::new();
                for r in body.iter().filter(|r| r.len() >= 5 && !r[0].is_empty() && (r[3] == "ok" || r[3] == "fix")) {
                    let mark = mark_of(&[("s", &r[3]), ("h", &r[4]), ("n", &text_of(r, 5)), ("d", &text_of(r, 6))]);
                    self.proof.entry(r[0].clone()).or_default().insert((r[1].clone(), norm_ws(&r[2])), mark);
                }
            }
            Kind::Ignored => {
                self.ignored = IndexMap::new();
                for r in body.iter().filter(|r| r.len() >= 5 && !r[0].is_empty() && !r[3].is_empty()) {
                    let mark = mark_of(&[("h", &r[4]), ("n", &text_of(r, 5)), ("d", &text_of(r, 6))]);
                    self.ignored.entry(r[0].clone()).or_default().insert((r[1].clone(), norm_ws(&r[2]), r[3].clone()), mark);
                }
            }
            Kind::Episodes => {
                self.episodes = IndexMap::new();
                for r in body.iter().filter(|r| r.len() >= 2) {
                    let digits: String = r[0].chars().filter(|c| c.is_ascii_digit()).collect();
                    let state = fold(&r[1]);
                    if let (Ok(n), Some(st)) = (digits.parse::<i64>(), EP_STATE_KEYS.iter().find(|k| **k == state)) {
                        self.episodes.insert(n, st.to_string());
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ save
    fn rows_of(&self, kind: Kind) -> Vec<Vec<String>> {
        let s = |x: &str| x.to_string();
        let mark_field = |m: &Mark, k: &str| m.get(k).map_or(String::new(), crate::messages::py_str);
        match kind {
            Kind::Corr => {
                let mut rows = vec![vec![s("file"), s("proofread"), s("translated_manual")]];
                for n in lower_sorted(self.corr.keys().collect()) {
                    let v = &self.corr[n];
                    rows.push(vec![n.clone(), v.c.clone(), v.t.clone()]);
                }
                rows
            }
            Kind::Assign => {
                let mut rows = vec![vec![s("file"), s("group")]];
                for n in lower_sorted(self.assign.iter().filter(|(_, v)| !v.is_empty()).map(|(k, _)| k).collect()) {
                    rows.push(vec![n.clone(), assign_to_text(&self.assign[n])]);
                }
                rows
            }
            Kind::Approved => {
                let mut data: Vec<(&String, &String, &String)> = self.approved.iter().flat_map(|(f, keys)| keys.iter().map(move |(b, o)| (f, b, o))).collect();
                data.sort();
                let mut rows = vec![vec![s("file"), s("block"), s("original")]];
                rows.extend(data.into_iter().map(|(f, b, o)| vec![f.clone(), b.clone(), o.clone()]));
                rows
            }
            Kind::Glossary => {
                let mut rows = vec![vec![s("code"), s("name"), s("type"), s("note")]];
                for code in lower_sorted(self.glossary.keys().collect()) {
                    let g = &self.glossary[code];
                    rows.push(vec![code.clone(), g.n.clone(), g.t.clone(), g.c.clone()]);
                }
                rows
            }
            Kind::Proof => {
                let mut rows = vec![["file", "block", "original", "state", "hash", "note", "date"].map(s).to_vec()];
                for f in lower_sorted(self.proof.keys().collect()) {
                    let mut items: Vec<_> = self.proof[f].iter().collect();
                    items.sort_by(|a, b| a.0.cmp(b.0));
                    for ((b, o), m) in items {
                        rows.push(vec![f.clone(), b.clone(), o.clone(), mark_field(m, "s"), mark_field(m, "h"), mark_field(m, "n"), mark_field(m, "d")]);
                    }
                }
                rows
            }
            Kind::Ignored => {
                let mut rows = vec![["file", "block", "original", "code", "hash", "note", "date"].map(s).to_vec()];
                for f in lower_sorted(self.ignored.keys().collect()) {
                    let mut items: Vec<_> = self.ignored[f].iter().collect();
                    items.sort_by(|a, b| a.0.cmp(b.0));
                    for ((b, o, code), m) in items {
                        rows.push(vec![f.clone(), b.clone(), o.clone(), code.clone(), mark_field(m, "h"), mark_field(m, "n"), mark_field(m, "d")]);
                    }
                }
                rows
            }
            Kind::Episodes => {
                let mut rows = vec![vec![s("episode"), s("state")]];
                for n in self.layout.active_groups() {
                    rows.push(vec![n.to_string(), self.episodes.get(&n).cloned().unwrap_or_else(|| s("none"))]);
                }
                rows
            }
        }
    }

    pub fn save(&mut self, kind: Kind, clock: &dyn Clock) -> Result<(), WriteFailed> {
        let rows = self.rows_of(kind);
        match write_csv(&self.path(kind), &rows, Some(&self.backups), clock) {
            Ok(()) => {
                self.unsaved.remove(kind.name());
                Ok(())
            }
            Err(e) => {
                self.unsaved.insert(kind.name());
                Err(e)
            }
        }
    }

    /// Add blank correction rows for new files; existing values (also of missing files) stay.
    pub fn ensure_files(&mut self, names: &[String], clock: &dyn Clock) {
        let new: Vec<&String> = names.iter().filter(|n| !self.corr.contains_key(*n)).collect();
        for n in &new {
            self.corr.insert((*n).clone(), Corr::default());
        }
        let head: Vec<Vec<String>> =
            if self.path(Kind::Corr).exists() { read_rows(&self.path(Kind::Corr)).unwrap_or_default().into_iter().take(1).collect() } else { Vec::new() };
        if !new.is_empty() || head.is_empty() || head[0].len() < 3 {
            if let Err(e) = self.save(Kind::Corr, clock) {
                self.warnings.push(plain(e.to_string()));
            }
        }
    }

    // ------------------------------------------------------------ event log (events.py)
    /// The shared state as {kind: {key: JSON value}}; empty rows (the blank lines added for new files) do not count.
    pub fn flatten(&self) -> IndexMap<&'static str, IndexMap<Key, Value>> {
        let mut out: IndexMap<&'static str, IndexMap<Key, Value>> = EVENT_KINDS.iter().map(|k| (*k, IndexMap::new())).collect();
        let k = K::s;
        for (f, v) in &self.corr {
            if !v.c.is_empty() || !v.t.is_empty() {
                out["corr"].insert(vec![k(f)], json!({"c": v.c, "t": v.t}));
            }
        }
        for (f, v) in &self.assign {
            if !v.is_empty() {
                out["assign"].insert(vec![k(f)], json!(v));
            }
        }
        for (f, keys) in &self.approved {
            for (b, o) in keys {
                out["approved"].insert(vec![k(f), k(b), k(o)], json!(true));
            }
        }
        for (n, st) in &self.episodes {
            out["episodes"].insert(vec![K::I(*n)], json!(st));
        }
        for (f, marks) in &self.proof {
            for ((b, o), m) in marks {
                out["proof"].insert(vec![k(f), k(b), k(o)], Value::Object(m.clone()));
            }
        }
        for (f, marks) in &self.ignored {
            for ((b, o, code), m) in marks {
                out["ignored"].insert(vec![k(f), k(b), k(o), k(code)], Value::Object(m.clone()));
            }
        }
        out
    }

    /// Apply one replayed value (null = removed) to the in-memory state. Returns true when something changed.
    fn set_flat(&mut self, kind: &str, key: &Key, value: &Value) -> bool {
        let removed = value.is_null();
        let text = |i: usize| match key.get(i) {
            Some(K::S(s)) => Some(s.clone()),
            _ => None,
        };
        match kind {
            "corr" => {
                let Some(file) = text(0) else { return false };
                let cur = self.corr.get(&file).cloned().unwrap_or_default();
                let field = |k: &str| value.get(k).map_or(String::new(), crate::messages::py_str);
                let new = if removed { Corr::default() } else { Corr { c: field("c"), t: field("t") } };
                if cur == new {
                    return false;
                }
                self.corr.insert(file, new);
            }
            "assign" => {
                let Some(file) = text(0) else { return false };
                let new = if removed { String::new() } else { value.as_str().unwrap_or("\u{0}").to_string() };
                if !removed && !valid_assign(&new, &self.layout) {
                    return false;
                }
                if self.assign.get(&file).map_or("", String::as_str) == new {
                    return false;
                }
                if removed {
                    self.assign.shift_remove(&file);
                } else {
                    self.assign.insert(file, new);
                }
            }
            "approved" => {
                let (Some(file), Some(b), Some(o)) = (text(0), text(1), text(2)) else { return false };
                let have = self.approved.get(&file).is_some_and(|s| s.contains(&(b.clone(), o.clone())));
                if have == !removed {
                    return false;
                }
                if removed {
                    if let Some(set) = self.approved.get_mut(&file) {
                        set.remove(&(b, o));
                        if set.is_empty() {
                            self.approved.shift_remove(&file);
                        }
                    }
                } else {
                    self.approved.entry(file).or_default().insert((b, o));
                }
            }
            "episodes" => {
                let Some(K::I(n)) = key.first() else { return false };
                let state = value.as_str().map(String::from);
                if !removed && !state.as_deref().is_some_and(|s| EP_STATE_KEYS.contains(&s)) {
                    return false;
                }
                if self.episodes.get(n).cloned() == state {
                    return false;
                }
                match state {
                    None => {
                        self.episodes.shift_remove(n);
                    }
                    Some(s) => {
                        self.episodes.insert(*n, s);
                    }
                }
            }
            "proof" => {
                let (Some(file), Some(b), Some(o)) = (text(0), text(1), text(2)) else { return false };
                let k = (b, o);
                let have = self.proof.get(&file).and_then(|t| t.get(&k)).map(|m| Value::Object(m.clone()));
                if have.as_ref().is_some_and(|h| h == value) || (have.is_none() && removed) {
                    return false;
                }
                if removed {
                    if let Some(t) = self.proof.get_mut(&file) {
                        t.shift_remove(&k);
                        if t.is_empty() {
                            self.proof.shift_remove(&file);
                        }
                    }
                } else if let Some(m) = value.as_object() {
                    self.proof.entry(file).or_default().insert(k, m.clone());
                } else {
                    return false;
                }
            }
            "ignored" => {
                let (Some(file), Some(b), Some(o), Some(c)) = (text(0), text(1), text(2), text(3)) else { return false };
                let k = (b, o, c);
                let have = self.ignored.get(&file).and_then(|t| t.get(&k)).map(|m| Value::Object(m.clone()));
                if have.as_ref().is_some_and(|h| h == value) || (have.is_none() && removed) {
                    return false;
                }
                if removed {
                    if let Some(t) = self.ignored.get_mut(&file) {
                        t.shift_remove(&k);
                        if t.is_empty() {
                            self.ignored.shift_remove(&file);
                        }
                    }
                } else if let Some(m) = value.as_object() {
                    self.ignored.entry(file).or_default().insert(k, m.clone());
                } else {
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    pub fn attach_events(&mut self, log: EventLog) -> &mut Store {
        self.events = Some(log);
        self
    }

    /// Changes (kind, key, value) that turn the flat state `base` into the current in-memory state.
    fn diff_to_events(&self, base: &IndexMap<String, IndexMap<Key, Value>>) -> Vec<(String, Key, Value)> {
        let now = self.flatten();
        let mut out = Vec::new();
        let empty = IndexMap::new();
        for kind in EVENT_KINDS {
            let (cur, old) = (&now[kind], base.get(kind).unwrap_or(&empty));
            out.extend(cur.iter().filter(|(k, v)| old.get(*k) != Some(*v)).map(|(k, v)| (kind.to_string(), k.clone(), v.clone())));
            out.extend(old.keys().filter(|k| !cur.contains_key(*k)).map(|k| (kind.to_string(), k.clone(), Value::Null)));
        }
        out
    }

    /// Write the difference between the memory and the log as events of this author (after every change and undo).
    pub fn record_changes(&mut self, clock: &dyn Clock) -> usize {
        let Some(log) = self.events.as_ref() else { return 0 };
        let changes = self.diff_to_events(&log.flat());
        match self.events.as_mut().unwrap().append(&changes, clock) {
            Ok(n) => n,
            Err(e) => {
                self.warnings.push(msg("The event log could not be written: {reason}", &[("reason", json!(e.to_string()))]));
                0
            }
        }
    }

    /// Reconcile the CSV state in memory with the event log. Returns the number of events of other authors that changed something.
    ///
    /// First run on a computer (no seen file): if the log is empty the current state is written as the first events; if the log already has events, it wins over the
    /// CSV files (they may be old). Later runs: differences between the memory (edited in Excel, say) and the part of the log read before become events of this
    /// author; then the events written by others since are merged in.
    pub fn sync_events(&mut self, clock: &dyn Clock) -> usize {
        let Some(log) = self.events.as_mut() else { return 0 };
        let (seen, unseen, ends) = log.read_all();
        log.state.clear();
        log.replay(&seen);
        let (known, has_events) = (log.known, !seen.is_empty() || !unseen.is_empty());
        if known || !has_events {
            self.record_changes(clock); // local edits since the last run (or the very first baseline)
        }
        let log = self.events.as_mut().unwrap();
        log.replay(&unseen);
        for (k, v) in ends {
            log.offsets.insert(k, v);
        }
        let mut changed: BTreeSet<&'static str> = BTreeSet::new();
        let kind_of = |name: &str| EVENT_KINDS.iter().copied().find(|k| *k == name);
        if !known && has_events {
            // the log is the truth: whatever the CSV files hold beyond it is old
            let have = log.flat();
            let mine = self.flatten();
            for (kind, slot) in mine {
                for key in slot.keys() {
                    if !have.get(kind).is_some_and(|h| h.contains_key(key)) && self.set_flat(kind, key, &Value::Null) {
                        changed.insert(kind);
                    }
                }
            }
        }
        let replayed: Vec<(String, Key, Value)> = self
            .events
            .as_ref()
            .unwrap()
            .state
            .iter()
            .flat_map(|(kind, slot)| slot.iter().map(move |(key, (value, _))| (kind.clone(), key.clone(), value.clone())))
            .collect();
        for (kind, key, value) in replayed {
            if let Some(k) = kind_of(&kind) {
                if self.set_flat(k, &key, &value) {
                    changed.insert(k);
                }
            }
        }
        let own = self.events.as_ref().unwrap().path.file_name().unwrap().to_string_lossy().into_owned();
        let merged = unseen.iter().filter(|(_, src)| *src != own).count();
        for name in &changed {
            if let Some(kind) = Kind::from_name(name) {
                if let Err(e) = self.save(kind, clock) {
                    self.warnings.push(plain(e.to_string()));
                }
            }
        }
        // keys that exist only in the memory (kept from the CSV) are events now, so the log is complete
        self.record_changes(clock);
        let log = self.events.as_mut().unwrap();
        log.known = true;
        log.save_seen(clock);
        if changed.is_empty() {
            0
        } else {
            merged
        }
    }

    // ------------------------------------------------------------ undoable change
    fn snapshot(&self, kind: Kind) -> Snapshot {
        match kind {
            Kind::Corr => Snapshot::Corr(self.corr.clone()),
            Kind::Assign => Snapshot::Assign(self.assign.clone()),
            Kind::Approved => Snapshot::Approved(self.approved.clone()),
            Kind::Episodes => Snapshot::Episodes(self.episodes.clone()),
            Kind::Proof => Snapshot::Proof(self.proof.clone()),
            Kind::Glossary => Snapshot::Glossary(self.glossary.clone()),
            Kind::Ignored => Snapshot::Ignored(self.ignored.clone()),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        match s {
            Snapshot::Corr(v) => self.corr = v,
            Snapshot::Assign(v) => self.assign = v,
            Snapshot::Approved(v) => self.approved = v,
            Snapshot::Episodes(v) => self.episodes = v,
            Snapshot::Proof(v) => self.proof = v,
            Snapshot::Glossary(v) => self.glossary = v,
            Snapshot::Ignored(v) => self.ignored = v,
        }
    }

    /// Run `f` (which mutates in-memory state), save `kinds`, and record an undo step. If saving fails, memory is rolled back and the error returned.
    pub fn change<F: FnOnce(&mut Store)>(&mut self, label: Label, kinds: &[Kind], f: F, clock: &dyn Clock) -> Result<(), WriteFailed> {
        let before: Vec<Snapshot> = kinds.iter().map(|k| self.snapshot(*k)).collect();
        f(self);
        for k in kinds {
            if let Err(e) = self.save(*k, clock) {
                for s in before {
                    self.restore(s);
                }
                return Err(e);
            }
        }
        let after: Vec<Snapshot> = kinds.iter().map(|k| self.snapshot(*k)).collect();
        if before != after {
            self.undo_stack.push(UndoStep { label, kinds: kinds.to_vec(), before });
            if self.undo_stack.len() > UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
            self.record_changes(clock);
        }
        Ok(())
    }

    /// Revert the most recent change. Returns its label, or `None` when there is nothing to undo.
    pub fn undo(&mut self, clock: &dyn Clock) -> Result<Option<Label>, WriteFailed> {
        let Some(step) = self.undo_stack.last().cloned() else { return Ok(None) };
        let current: Vec<Snapshot> = step.kinds.iter().map(|k| self.snapshot(*k)).collect();
        for s in step.before.iter().cloned() {
            self.restore(s);
        }
        for k in &step.kinds {
            if let Err(e) = self.save(*k, clock) {
                for s in current {
                    self.restore(s);
                }
                return Err(e);
            }
        }
        self.undo_stack.pop();
        self.record_changes(clock);
        Ok(Some(step.label))
    }

    pub fn clear_undo(&mut self) {
        self.undo_stack.clear();
    }
}

fn mark_of(fields: &[(&str, &str)]) -> Mark {
    fields.iter().map(|(k, v)| (k.to_string(), json!(v))).collect()
}
