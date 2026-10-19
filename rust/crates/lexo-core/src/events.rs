//! Port of `reference/python/core/lexo/events.py`: the event log, every change of the shared project state, one append-only file per author (docs/technical/DATA-LAYOUT.md, docs/product/TEAMWORK.md).
//!
//! `<translation folder>/.aside/events/<author>.jsonl` holds one JSON object per line, written only by that author; `<local folder>/events-seen.json` says how many bytes
//! of each file this computer has already read. An event says "this key of this kind now has this value" (`value` null = removed); the state is the replay of all
//! events of all authors; for one key the newest event wins (time, then author id, then position). Undo is a new event, never a deleted line; a damaged line is skipped.
use crate::clock::Clock;
use crate::fsutil::atomic_write_text;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const VERSION: i64 = 1;
pub const SEEN_FILE: &str = "events-seen.json";

/// A safe file name for an author id.
pub fn clean_author(name: &str) -> String {
    let lowered = crate::util::py_strip(name).to_lowercase();
    let mut out = String::new();
    let mut in_run = false;
    for c in lowered.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    let cleaned: String = out.trim_matches('-').chars().take(40).collect();
    if cleaned.is_empty() {
        "author".into()
    } else {
        cleaned
    }
}

/// One element of the key of an event: text, a whole number, or any other JSON scalar as its text.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum K {
    S(String),
    I(i64),
    Other(String),
}

pub type Key = Vec<K>;

impl K {
    pub fn to_value(&self) -> Value {
        match self {
            K::S(s) => json!(s),
            K::I(i) => json!(i),
            K::Other(t) => serde_json::from_str(t).unwrap_or(Value::Null),
        }
    }
    pub fn s(text: &str) -> K {
        K::S(text.to_string())
    }
}

/// The key of an event, or `None` when an element cannot be part of a key (a list or a dict: Python's `hash` fails).
pub fn key_of(values: &[Value]) -> Option<Key> {
    values
        .iter()
        .map(|v| match v {
            Value::String(s) => Some(K::S(s.clone())),
            Value::Number(n) if n.is_i64() => Some(K::I(n.as_i64().unwrap())),
            Value::Array(_) | Value::Object(_) => None,
            other => Some(K::Other(other.to_string())),
        })
        .collect()
}

pub fn key_json(key: &Key) -> Value {
    Value::Array(key.iter().map(K::to_value).collect())
}

/// (time, author, position): the newest wins.
pub type Stamp = (String, String, usize);

pub type Event = Map<String, Value>;

/// What `read_all` finds: the events already seen, the events not seen yet, and where each file ends.
pub type Replay = (Vec<(Event, String)>, Vec<(Event, String)>, IndexMap<String, u64>);

/// `kind -> key -> (value, stamp)`
pub type State = IndexMap<String, IndexMap<Key, (Value, Stamp)>>;

fn py_str(v: &Value) -> String {
    crate::messages::py_str(v)
}

pub struct EventLog {
    pub dir: PathBuf,
    seen_path: PathBuf,
    pub author: String,
    pub path: PathBuf,
    /// The replay of everything read so far.
    pub state: State,
    /// File name -> bytes already read (and applied to `state`).
    pub offsets: IndexMap<String, u64>,
    /// Was there a seen file (false = first run of this computer on this project).
    pub known: bool,
    last_t: String,
}

impl EventLog {
    pub fn new(directory: &Path, seen_path: &Path, author: &str) -> EventLog {
        let author = clean_author(author);
        let mut log = EventLog {
            dir: directory.to_path_buf(),
            seen_path: seen_path.to_path_buf(),
            path: directory.join(format!("{author}.jsonl")),
            author,
            state: State::new(),
            offsets: IndexMap::new(),
            known: false,
            last_t: String::new(),
        };
        log.load_seen();
        log
    }

    // ------------------------------------------------------------ the local "seen" file
    fn load_seen(&mut self) {
        let parsed = std::fs::read_to_string(&self.seen_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| {
            let offsets = match v.as_object()?.get("offsets") {
                None => Map::new(),
                Some(o) => o.as_object()?.clone(),
            };
            offsets
                .iter()
                .map(|(k, v)| {
                    let n = v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))?;
                    Some((k.clone(), n.max(0) as u64))
                })
                .collect::<Option<IndexMap<String, u64>>>()
        });
        match parsed {
            Some(o) => {
                self.offsets = o;
                self.known = true;
            }
            None => {
                self.offsets = IndexMap::new();
                self.known = false;
            }
        }
    }

    pub fn save_seen(&self, clock: &dyn Clock) {
        let offsets: Map<String, Value> = self.offsets.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let doc = json!({"v": VERSION, "offsets": offsets});
        // `json.dumps(..., indent=1)`: one space per level
        let mut buf = Vec::new();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
        if doc.serialize(&mut ser).is_ok() {
            // a failed note only means more is read next time
            let _ = atomic_write_text(&self.seen_path, &String::from_utf8_lossy(&buf), None, None, clock);
        }
    }

    // ------------------------------------------------------------ reading
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file()).collect(),
            Err(_) => Vec::new(),
        };
        out.sort();
        out
    }

    /// The complete lines from `start` as (event, end offset); a damaged line is skipped. Also where the complete part ends.
    fn read_file(path: &Path, start: u64) -> (Vec<(Event, u64)>, u64) {
        let Ok(mut f) = std::fs::File::open(path) else { return (Vec::new(), start) };
        let mut data = Vec::new();
        if f.seek(SeekFrom::Start(start)).is_err() || f.read_to_end(&mut data).is_err() {
            return (Vec::new(), start);
        }
        let cut = data.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        let mut out = Vec::new();
        let mut pos = start;
        if cut > 0 {
            for raw in data[..cut - 1].split(|b| *b == b'\n') {
                pos += raw.len() as u64 + 1;
                let Ok(text) = std::str::from_utf8(raw) else { continue };
                let Ok(Value::Object(ev)) = serde_json::from_str::<Value>(text) else { continue };
                let kind_ok = ev.get("kind").is_some_and(crate::util::truthy);
                if kind_ok && ev.get("key").is_some_and(Value::is_array) && ev.get("t").is_some_and(Value::is_string) {
                    out.push((ev, pos));
                }
            }
        }
        (out, start + cut as u64)
    }

    /// Every event of every file, split into (seen, unseen) by the saved offsets, and where each file ends.
    pub fn read_all(&self) -> Replay {
        let (mut seen, mut unseen, mut ends) = (Vec::new(), Vec::new(), IndexMap::new());
        for path in self.files() {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let (evs, end) = Self::read_file(&path, 0);
            let limit = if self.known { self.offsets.get(&name).copied().unwrap_or(0) } else { 0 };
            for (ev, pos) in evs {
                if pos <= limit { &mut seen } else { &mut unseen }.push((ev, name.clone()));
            }
            ends.insert(name, end);
        }
        (seen, unseen, ends)
    }

    /// Events written since the last read (any author); advances the offsets.
    pub fn read_new(&mut self) -> Vec<(Event, String)> {
        let mut fresh = Vec::new();
        let mut ends = Vec::new();
        for path in self.files() {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let (evs, end) = Self::read_file(&path, self.offsets.get(&name).copied().unwrap_or(0));
            fresh.extend(evs.into_iter().map(|(ev, _)| (ev, name.clone())));
            ends.push((name, end));
        }
        for (k, v) in ends {
            self.offsets.insert(k, v);
        }
        fresh
    }

    // ------------------------------------------------------------ replay
    /// Merge [(event, file name)] into `state` (newest wins per key).
    pub fn replay_into(state: &mut State, items: &[(Event, String)]) {
        for (n, (ev, source)) in items.iter().enumerate() {
            let stamp: Stamp = (ev.get("t").and_then(Value::as_str).unwrap_or("").to_string(), ev.get("a").map_or_else(|| source.clone(), py_str), n);
            let Some(key) = ev.get("key").and_then(Value::as_array).and_then(|k| key_of(k)) else { continue };
            let slot = state.entry(py_str(&ev["kind"])).or_default();
            let newer = slot.get(&key).is_none_or(|old| stamp >= old.1);
            if newer {
                slot.insert(key, (ev.get("value").cloned().unwrap_or(Value::Null), stamp));
            }
        }
    }

    pub fn replay(&mut self, items: &[(Event, String)]) {
        Self::replay_into(&mut self.state, items);
    }

    /// `{kind: {key: value}}` without removed keys.
    pub fn flat(&self) -> IndexMap<String, IndexMap<Key, Value>> {
        self.state
            .iter()
            .map(|(k, slot)| (k.clone(), slot.iter().filter(|(_, (v, _))| !v.is_null()).map(|(key, (v, _))| (key.clone(), v.clone())).collect()))
            .collect()
    }

    // ------------------------------------------------------------ writing
    /// The time of a new event: strictly increasing within this process.
    fn now(&mut self, clock: &dyn Clock) -> String {
        let fmt = |t: DateTime<Utc>| t.format("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string();
        let mut s = fmt(clock.now_utc());
        if s <= self.last_t {
            let last = DateTime::parse_from_rfc3339(&self.last_t).map(|d| d.with_timezone(&Utc)).unwrap_or_default();
            s = fmt(last + chrono::Duration::microseconds(1));
        }
        self.last_t = s.clone();
        s
    }

    /// Write [(kind, key, value or null)] as events of this author and merge them into the state. Returns the number written.
    pub fn append(&mut self, changes: &[(String, Key, Value)], clock: &dyn Clock) -> std::io::Result<usize> {
        if changes.is_empty() {
            return Ok(0);
        }
        std::fs::create_dir_all(&self.dir)?;
        let name = self.path.file_name().unwrap().to_string_lossy().into_owned();
        let (mut lines, mut items) = (Vec::new(), Vec::new());
        for (kind, key, value) in changes {
            let mut ev = Map::new();
            ev.insert("v".into(), json!(VERSION));
            ev.insert("t".into(), json!(self.now(clock)));
            ev.insert("a".into(), json!(self.author));
            ev.insert("kind".into(), json!(kind));
            ev.insert("key".into(), key_json(key));
            ev.insert("value".into(), value.clone());
            lines.push(Value::Object(ev.clone()).to_string());
            items.push((ev, name.clone()));
        }
        let mut lead: &[u8] = b"";
        if let Ok(meta) = std::fs::metadata(&self.path) {
            // a previous write was cut short: never glue two lines
            if meta.len() > 0 && !std::fs::read(&self.path)?.ends_with(b"\n") {
                lead = b"\n";
            }
        }
        let mut blob = lead.to_vec();
        blob.extend((lines.join("\n") + "\n").into_bytes());
        std::fs::OpenOptions::new().append(true).create(true).open(&self.path)?.write_all(&blob)?;
        self.replay(&items);
        self.offsets.insert(name, std::fs::metadata(&self.path)?.len());
        self.save_seen(clock);
        Ok(lines.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_ids_become_safe_file_names() {
        assert_eq!(clean_author("Bob Smith"), "bob-smith");
        assert_eq!(clean_author("  "), "author");
        assert_eq!(clean_author("a/b\\c"), "a-b-c");
        assert_eq!(clean_author("---x---"), "x");
        assert_eq!(clean_author(&"x".repeat(60)).len(), 40);
    }

    #[test]
    fn a_key_with_a_list_in_it_is_not_a_key() {
        assert!(key_of(&[json!("a"), json!(["x"])]).is_none());
        assert_eq!(key_of(&[json!("a"), json!(1)]), Some(vec![K::s("a"), K::I(1)]));
    }
}
