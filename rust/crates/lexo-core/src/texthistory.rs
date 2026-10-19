//! Port of `reference/python/core/lexo/texthistory.py`: the history of the translated text of every line.
//!
//! Each change of a translation is appended to `text_history.jsonl` in the data folder: when, which line (file + the key that proofreading uses: block + normalised
//! original, so it follows the line when the file is reorganised), the old and the new text, and how it happened. The translator can look at all versions of a line and
//! restore one. The file is append-only; when it grows beyond `MAX_LINES` the oldest entries are dropped. It is written with `\n` line ends on every system.
use crate::clock::Clock;
use crate::messages::msg;
use crate::pyfmt::json_dumps;
use crate::rpy::Entry;
use crate::store::Label;
use crate::util::{norm_ws, py_splitlines};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "text_history.jsonl";
pub const MAX_LINES: usize = 60000;
pub const KEEP_LINES: usize = 40000;

/// How a change happened; the labels are messages (a key that is not known is shown as it is).
pub fn via_label(key: &str) -> Label {
    let m = |t: &str| Label::Msg(msg(t, &[]));
    match key {
        "edit" => m("edit in the app"),
        "fix" => m("fix"),
        "replace" => m("replace"),
        "unify" => m("unify"),
        "undo" => m("undo"),
        "restore" => m("restore from a backup"),
        "external" => m("change outside the app"),
        other => Label::Plain(other.to_string()),
    }
}

pub fn key_of(block: &str, source: &str) -> [String; 2] {
    [block.to_string(), norm_ws(source)]
}

pub type Record = Map<String, Value>;

pub struct TextHistory {
    pub path: PathBuf,
    /// (file, block, source) -> records
    pub by_key: IndexMap<(String, String, String), Vec<Record>>,
    pub total: usize,
}

fn text(v: &Value) -> String {
    crate::messages::py_str(v)
}

impl TextHistory {
    pub fn new(path: &Path) -> TextHistory {
        TextHistory { path: path.to_path_buf(), by_key: IndexMap::new(), total: 0 }
    }

    pub fn load(&mut self) -> &mut TextHistory {
        self.load_with(MAX_LINES, KEEP_LINES)
    }

    /// `load` with the limits given (the golden data uses small ones).
    pub fn load_with(&mut self, max_lines: usize, keep_lines: usize) -> &mut TextHistory {
        self.by_key.clear();
        self.total = 0;
        let Ok(bytes) = std::fs::read(&self.path) else { return self };
        let mut lines = py_splitlines(&String::from_utf8_lossy(&bytes));
        if lines.len() > max_lines {
            // keep the newest part, once, at load time
            lines = lines.split_off(lines.len() - keep_lines);
            let _ = std::fs::write(&self.path, lines.join("\n") + "\n");
        }
        for ln in &lines {
            let Ok(Value::Object(r)) = serde_json::from_str::<Value>(ln) else { continue };
            let (Some(f), Some(Value::Array(k))) = (r.get("f"), r.get("k")) else { continue };
            if k.len() < 2 {
                continue; // a damaged line never breaks the history
            }
            self.by_key.entry((text(f), text(&k[0]), text(&k[1]))).or_default().push(r.clone());
            self.total += 1;
        }
        self
    }

    /// items: [(file, (block, source), old, new)]. Entries with an unchanged text are ignored. Returns the number of lines written.
    pub fn record(&mut self, items: &[(String, (String, String), String, String)], via: &str, clock: &dyn Clock) -> usize {
        let now = clock.now_local().format("%Y-%m-%dT%H:%M:%S").to_string();
        let mut rows: Vec<Record> = Vec::new();
        for (file, (blk, src), old, new) in items {
            if old == new {
                continue;
            }
            let mut r = Map::new();
            r.insert("t".into(), json!(now));
            r.insert("f".into(), json!(file));
            r.insert("k".into(), json!([blk, norm_ws(src)]));
            r.insert("o".into(), json!(old));
            r.insert("n".into(), json!(new));
            r.insert("v".into(), json!(via));
            rows.push(r);
        }
        if rows.is_empty() {
            return 0;
        }
        let blob: String = rows.iter().map(|r| json_dumps(&Value::Object(r.clone()), false) + "\n").collect();
        let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(&self.path) else { return 0 };
        if f.write_all(blob.as_bytes()).is_err() {
            return 0;
        }
        for r in &rows {
            let k = r["k"].as_array().unwrap();
            self.by_key.entry((text(&r["f"]), text(&k[0]), text(&k[1]))).or_default().push(r.clone());
        }
        self.total += rows.len();
        rows.len()
    }

    /// Texts this line has had, newest first: [{text, time, via, current}]. The text before the first recorded change is included as the oldest version.
    pub fn versions(&self, file: &str, key: (&str, &str), current: &str) -> Vec<Value> {
        let Some(recs) = self.by_key.get(&(file.to_string(), key.0.to_string(), norm_ws(key.1))).filter(|r| !r.is_empty()) else { return Vec::new() };
        let mut seq: Vec<(String, String, Label)> =
            vec![(text(&recs[0]["o"]), String::new(), Label::Msg(msg("original text before the first recorded change", &[])))];
        for r in recs {
            seq.push((text(&r["n"]), text(&r["t"]), via_label(&text(&r["v"]))));
        }
        let mut out: Vec<Value> = Vec::new();
        let mut any_current = false;
        for (t, time, via) in seq.into_iter().rev() {
            let is_current = t == current && !any_current;
            any_current |= is_current;
            out.push(json!({"text": t, "time": time, "via": via.wire(), "current": is_current}));
        }
        out.truncate(60);
        out
    }
}

/// A changed translation between two parses of the same file.
#[derive(Debug, PartialEq)]
pub struct Change {
    pub key: (String, String),
    pub old: String,
    pub new: String,
    pub block: String,
    pub source: Option<String>,
}

impl Change {
    pub fn to_json(&self) -> Value {
        json!({"key": [self.key.0, self.key.1], "old": self.old, "new": self.new, "block": self.block, "source": self.source})
    }
}

/// Changed translations between two parses of the same file (matched by key and order).
pub fn diff_files(old_entries: &[Entry], new_entries: &[Entry]) -> Vec<Change> {
    let mut seen: IndexMap<(String, String), Vec<&str>> = IndexMap::new();
    for e in old_entries {
        seen.entry(e.key()).or_default().push(&e.target);
    }
    let mut used: IndexMap<(String, String), usize> = IndexMap::new();
    let mut out = Vec::new();
    for e in new_entries {
        let key = e.key();
        let Some(olds) = seen.get(&key).filter(|o| !o.is_empty()) else { continue };
        let i = *used.get(&key).unwrap_or(&0);
        used.insert(key.clone(), i + 1);
        if i < olds.len() && olds[i] != e.target {
            out.push(Change { key, old: olds[i].to_string(), new: e.target.clone(), block: e.block.clone(), source: e.source.clone() });
        }
    }
    out
}
