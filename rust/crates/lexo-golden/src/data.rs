//! The data scenarios of `core/golden/data` (docs/GOLDEN.md): a few files, a fixed clock and a list of steps, each calling one function of Core; the recorded answer
//! of every step and the bytes of every file at the end are compared. This is the same runner as `run_data_scenario` in `core/tests/golden_tool.py`.
use crate::normalize::normalize;
use chrono::{DateTime, NaiveDateTime, Utc};
use lexo_core::clock::{Clock, TestClock};
use lexo_core::config::Config;
use lexo_core::lock::{Owner, ProjectLock};
use lexo_core::util::sha1_hex;
use lexo_core::{csvio, datadir, fsutil};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| *c != b'=') {
        let v = B64.iter().position(|b| *b == c).unwrap_or(0) as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xFF) as u8);
        }
    }
    out
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(|s| s.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default()
}

fn rows(v: &Value) -> Vec<Vec<String>> {
    v.as_array().map(|a| a.iter().map(strings).collect()).unwrap_or_default()
}

fn file_records(root: &Path, skip: &[&str]) -> Map<String, Value> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
    let mut all = Vec::new();
    walk(root, &mut all);
    all.sort();
    let mut files = Map::new();
    for f in all {
        if skip.contains(&f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().as_str()) {
            continue;
        }
        let b = std::fs::read(&f).unwrap_or_default();
        let mut rec = Map::new();
        rec.insert("sha1".into(), json!(sha1_hex(&b)));
        match String::from_utf8(b.clone()) {
            Ok(t) => rec.insert("text".into(), json!(t)),
            Err(_) => rec.insert("base64".into(), json!(base64_encode(&b))),
        };
        files.insert(f.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), Value::Object(rec));
    }
    files
}

/// Random by design: a token, a time, a project id.
const SKIP: [&str; 2] = ["lock.json", "project-id"];

struct Runner<'a> {
    root: &'a Path,
    cfg_dir: PathBuf,
    clock: &'a TestClock,
    locks: HashMap<String, ProjectLock>,
    config: Option<Config>,
}

fn err_text(e: &lexo_core::fsutil::WriteFailed) -> String {
    e.to_string()
}

impl Runner<'_> {
    fn p(&self, v: &Value) -> PathBuf {
        self.root.join(v.as_str().unwrap_or(""))
    }

    fn backup_dir(&self, a: &Value) -> Option<PathBuf> {
        a.get("backup_dir").and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| self.root.join(s))
    }

    /// `Ok(answer)` or `Err(message)` for an error Core reports itself (a failed write).
    fn run(&mut self, op: &str, a: &Value) -> Result<Value, String> {
        let clock: &dyn Clock = self.clock;
        Ok(match op {
            "advance" => {
                self.clock.advance(a["seconds"].as_f64().unwrap_or(0.0));
                json!(true)
            }
            "set_epoch" => {
                self.clock.set_epoch(a["value"].as_f64());
                json!(true)
            }
            "csv.read" => json!(fsutil::read_rows(&self.p(&a["path"])).map_err(|e| e.to_string())?),
            "csv.text" => json!(csvio::csv_text(&rows(&a["rows"]), a["delimiter"].as_str().and_then(|d| d.chars().next()).unwrap_or(';'))),
            "write_csv" => {
                fsutil::write_csv(&self.p(&a["path"]), &rows(&a["rows"]), self.backup_dir(a).as_deref(), clock).map_err(|e| err_text(&e))?;
                json!(true)
            }
            "atomic_write" => {
                fsutil::atomic_write_text(&self.p(&a["path"]), a["text"].as_str().unwrap_or(""), a["newline"].as_str(), self.backup_dir(a).as_deref(), clock)
                    .map_err(|e| err_text(&e))?;
                json!(true)
            }
            "make_backup" => {
                let keep = a["keep"].as_u64().unwrap_or(20) as usize;
                let got = fsutil::make_backup(&self.p(&a["path"]), &self.p(&a["backup_dir"]), keep, a["min_interval"].as_f64().unwrap_or(300.0), clock);
                json!(got.map(|p| p.file_name().unwrap().to_string_lossy().into_owned()))
            }
            "list_dir" => {
                let d = self.p(&a["path"]);
                if d.is_dir() {
                    let mut names: Vec<String> = std::fs::read_dir(&d)
                        .map_err(|e| e.to_string())?
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect();
                    names.sort();
                    json!(names)
                } else {
                    Value::Null
                }
            }
            "text_match" => match fsutil::text_matcher(a["query"].as_str().unwrap_or(""), a["whole"].as_bool().unwrap_or(false)) {
                None => Value::Null,
                Some(f) => json!(strings(&a["texts"]).iter().map(|t| f(t)).collect::<Vec<_>>()),
            },
            "config.load" => {
                let mut c = Config::new(Some(&self.cfg_dir.join("config.json")));
                c.load();
                let answer = json!({
                    "data": c.data,
                    "load_error": c.load_error.as_ref().map(|m| m.text().split(" (").next().unwrap_or("").to_string()).unwrap_or_default(),
                    "public": c.public(),
                });
                self.config = Some(c);
                answer
            }
            "config.set" => {
                let c = self.config.as_mut().ok_or("no config loaded")?;
                c.set(a["key"].as_str().unwrap_or(""), a["value"].clone());
                c.get(a["key"].as_str().unwrap_or("")).cloned().unwrap_or(Value::Null)
            }
            "config.get" => {
                let c = self.config.as_ref().ok_or("no config loaded")?;
                c.get(a["key"].as_str().unwrap_or("")).cloned().unwrap_or_else(|| a.get("default").cloned().unwrap_or(Value::Null))
            }
            "config.update" => {
                let c = self.config.as_mut().ok_or("no config loaded")?;
                c.update(&a["patch"], clock).map_err(|e| err_text(&e))?;
                c.data.clone()
            }
            "datadir.project_id" => {
                let shared = self.p(&a["shared"]);
                let pid = datadir::project_id(&shared, clock);
                json!({"ok": pid.chars().all(char::is_alphanumeric) && pid.chars().count() >= 8, "again": datadir::project_id(&shared, clock) == pid})
            }
            "datadir.local_dir" => json!(datadir::local_dir(&self.p(&a["shared"]), &self.p(&a["root"]), &self.cfg_dir, clock).to_string_lossy()),
            "lock.acquire" => {
                let name = a["name"].as_str().unwrap_or("").to_string();
                let owner = Owner { pid: a["pid"].as_i64().unwrap_or(1), host: a["host"].as_str().unwrap_or("pc").to_string() };
                let mut lk = self.locks.remove(&name).unwrap_or_else(|| ProjectLock::new(&self.p(&a["dir"]), owner.clone()));
                lk.owner = owner;
                if let Some(t) = a["token"].as_str() {
                    lk.token = t.to_string();
                }
                match lk.acquire(a["force"].as_bool().unwrap_or(false), clock) {
                    Err(e) => json!({"locked": true, "holder": e.holder, "message": e.message.text()}),
                    Ok(()) => {
                        self.locks.insert(name, lk);
                        json!({"locked": false})
                    }
                }
            }
            "lock.check" => json!(self.locks.get_mut(a["name"].as_str().unwrap_or("")).ok_or("no such lock")?.check()),
            "lock.release" => {
                let mut lk = self.locks.remove(a["name"].as_str().unwrap_or("")).ok_or("no such lock")?;
                lk.release();
                json!(!lk.held)
            }
            other => return Err(format!("unknown data step {other}")),
        })
    }
}

fn roots(base: &Path) -> Vec<String> {
    let canon = std::fs::canonicalize(base).map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string()).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for p in [base.to_string_lossy().into_owned(), canon] {
        if p.is_empty() {
            continue;
        }
        for s in [p.clone(), p.replace('\\', "/"), p.replace('\\', "\\\\")] {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.len()));
    out
}

fn temp_dir(prefix: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("{prefix}{}-{}", std::process::id(), lexo_core::fsutil::token_hex(4)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Run one data scenario (`core/golden/data/<name>.json`) and give its normalised answers: `{"steps": [...], "files_after": {...}}`.
pub fn run_data_scenario(path: &Path) -> Result<Value, String> {
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| e.to_string())?;
    let tmp = temp_dir("lexo-golden-");
    let result = (|| {
        for (rel, content) in spec["files"].as_object().into_iter().flatten() {
            let p = tmp.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
            let bytes = match content.get("base64").and_then(Value::as_str) {
                Some(b) => base64_decode(b),
                None => content.as_str().unwrap_or("").as_bytes().to_vec(),
            };
            std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
        }
        let c = &spec["clock"];
        let local: NaiveDateTime = c["local"].as_str().unwrap_or("2026-10-17T11:00:00").parse().map_err(|e| format!("{e}"))?;
        let utc: DateTime<Utc> =
            DateTime::parse_from_rfc3339(c["utc"].as_str().unwrap_or("2026-10-17T09:00:00+00:00")).map_err(|e| e.to_string())?.with_timezone(&Utc);
        let clock = TestClock::new(local, utc);
        let mut runner = Runner { root: &tmp, cfg_dir: tmp.join("cfg"), clock: &clock, locks: HashMap::new(), config: None };
        let mut steps = Vec::new();
        for st in spec["steps"].as_array().ok_or("no steps")? {
            let op = st["op"].as_str().unwrap_or("");
            let params: Map<String, Value> = st.as_object().unwrap().iter().filter(|(k, _)| *k != "op").map(|(k, v)| (k.clone(), v.clone())).collect();
            let mut entry = Map::new();
            entry.insert("op".into(), json!(op));
            entry.insert("params".into(), Value::Object(params));
            match runner.run(op, st) {
                Ok(v) => entry.insert("answer".into(), v),
                Err(e) => entry.insert("error".into(), json!(e)),
            };
            steps.push(Value::Object(entry));
        }
        for lk in runner.locks.values_mut() {
            lk.release();
        }
        drop(runner);
        Ok(normalize(&json!({"steps": steps, "files_after": file_records(&tmp, &SKIP)}), &roots(&tmp)))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}
