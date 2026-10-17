//! The data scenarios of `core/golden/data` (docs/GOLDEN.md): a few files, a fixed clock and a list of steps, each calling one function of Core; the recorded answer
//! of every step and the bytes of every file at the end are compared. This is the same runner as `run_data_scenario` in `core/tests/golden_tool.py`.
use crate::normalize::normalize;
use chrono::{DateTime, NaiveDateTime, Utc};
use lexo_core::clock::{Clock, TestClock};
use lexo_core::config::Config;
use lexo_core::events::{self, EventLog};
use lexo_core::history::{self, History};
use lexo_core::layout::{self, Layout};
use lexo_core::lock::{Owner, ProjectLock};
use lexo_core::memory::{self, Memory};
use lexo_core::messages::render;
use lexo_core::snapshots;
use lexo_core::store::{self, Kind, Store};
use lexo_core::texthistory::{self, TextHistory};
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
        let mut b = std::fs::read(&f).unwrap_or_default();
        if f.extension().is_some_and(|x| x == "zip") {
            // the bytes of a zip depend on the compressor: its files and their digests are compared
            let name = f.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let mut entries: Vec<Value> = Vec::new();
            if let Ok(mut z) = zip::ZipArchive::new(std::io::Cursor::new(b)) {
                for i in 0..z.len() {
                    if let Ok(mut e) = z.by_index(i) {
                        let mut data = Vec::new();
                        let _ = std::io::Read::read_to_end(&mut e, &mut data);
                        entries.push(json!([e.name(), sha1_hex(&data)]));
                    }
                }
            }
            entries.sort_by_key(|v| v.to_string());
            files.insert(name, json!({"zip": entries}));
            continue;
        }
        if f.file_name().is_some_and(|n| n == "text_history.jsonl") {
            // Python writes it in text mode (CRLF on Windows, LF elsewhere); this program always writes LF
            b = String::from_utf8_lossy(&b).replace("\r\n", "\n").into_bytes();
        }
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
    layout: Layout,
    store: Option<Store>,
    log: Option<EventLog>,
    hist: Option<History>,
    th: Option<TextHistory>,
    mem: Option<Memory>,
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
            "layout.use" => {
                self.layout = layout::parse(&a["raw"]).map_err(|e| e.to_string())?;
                self.layout.to_json()
            }
            "store.open" => {
                self.store = Some(Store::new(&self.p(&a["shared"]), Some(&self.p(&a["local"])), self.layout.clone()));
                json!(true)
            }
            "store.load" => {
                self.store()?.load();
                self.dump_store()?
            }
            "store.dump" => self.dump_store()?,
            "store.assign" => {
                let (file, value) = (a["file"].as_str().unwrap_or("").to_string(), a["value"].as_str().unwrap_or("").to_string());
                self.change(Kind::Assign, move |st| {
                    if value.is_empty() {
                        st.assign.shift_remove(&file);
                    } else {
                        st.assign.insert(file, value);
                    }
                })?
            }
            "store.corr" => {
                let (file, c, t) =
                    (a["file"].as_str().unwrap_or("").to_string(), a["c"].as_str().unwrap_or("").to_string(), a["t"].as_str().unwrap_or("").to_string());
                self.change(Kind::Corr, move |st| {
                    st.corr.insert(file, store::Corr { c, t });
                })?
            }
            "store.approve" => {
                let file = a["file"].as_str().unwrap_or("").to_string();
                let key = (a["block"].as_str().unwrap_or("").to_string(), lexo_core::util::norm_ws(a["original"].as_str().unwrap_or("")));
                let on = a["on"].as_bool().unwrap_or(false);
                self.change(Kind::Approved, move |st| {
                    if on {
                        st.approved.entry(file).or_default().insert(key);
                    } else {
                        if let Some(set) = st.approved.get_mut(&file) {
                            set.remove(&key);
                        }
                        if st.approved.get(&file).is_some_and(|s| s.is_empty()) {
                            st.approved.shift_remove(&file);
                        }
                    }
                })?
            }
            "store.episode" => {
                let n = a["n"].as_i64().unwrap_or(0);
                let state = a["state"].as_str().map(String::from);
                self.change(Kind::Episodes, move |st| match state {
                    None => {
                        st.episodes.shift_remove(&n);
                    }
                    Some(s) => {
                        st.episodes.insert(n, s);
                    }
                })?
            }
            "store.proof" | "store.ignored" => {
                let file = a["file"].as_str().unwrap_or("").to_string();
                let block = a["block"].as_str().unwrap_or("").to_string();
                let orig = lexo_core::util::norm_ws(a["original"].as_str().unwrap_or(""));
                let mark = a["mark"].as_object().cloned();
                if op == "store.proof" {
                    self.change(Kind::Proof, move |st| match mark {
                        None => {
                            if let Some(t) = st.proof.get_mut(&file) {
                                t.shift_remove(&(block, orig));
                            }
                            if st.proof.get(&file).is_some_and(|t| t.is_empty()) {
                                st.proof.shift_remove(&file);
                            }
                        }
                        Some(m) => {
                            st.proof.entry(file).or_default().insert((block, orig), m);
                        }
                    })?
                } else {
                    let code = a["code"].as_str().unwrap_or("").to_string();
                    self.change(Kind::Ignored, move |st| match mark {
                        None => {
                            if let Some(t) = st.ignored.get_mut(&file) {
                                t.shift_remove(&(block, orig, code));
                            }
                            if st.ignored.get(&file).is_some_and(|t| t.is_empty()) {
                                st.ignored.shift_remove(&file);
                            }
                        }
                        Some(m) => {
                            st.ignored.entry(file).or_default().insert((block, orig, code), m);
                        }
                    })?
                }
            }
            "store.glossary" => {
                let code = a["code"].as_str().unwrap_or("").to_string();
                let remove = a["remove"].as_bool().unwrap_or(false);
                let g =
                    store::Gloss { n: a["n"].as_str().unwrap_or("").into(), t: a["t"].as_str().unwrap_or("").into(), c: a["c"].as_str().unwrap_or("").into() };
                self.change(Kind::Glossary, move |st| {
                    if remove {
                        st.glossary.shift_remove(&code);
                    } else {
                        st.glossary.insert(code, g);
                    }
                })?
            }
            "store.undo" => {
                let label = self.store()?.undo(clock).map_err(|e| err_text(&e))?;
                json!({"label": label, "state": self.dump_store()?})
            }
            "store.ensure_files" => {
                self.store()?.ensure_files(&strings(&a["names"]), clock);
                self.dump_store()?
            }
            "store.attach_events" => {
                let log = EventLog::new(&self.p(&a["dir"]), &self.p(&a["seen"]), a["author"].as_str().unwrap_or(""));
                self.store()?.attach_events(log);
                json!(true)
            }
            "store.sync_events" => {
                let merged = self.store()?.sync_events(clock);
                let log = self.store()?.events.as_ref().ok_or("no event log")?;
                let (offsets, known) = (offsets_json(&log.offsets), log.known);
                json!({"merged": merged, "state": self.dump_store()?, "offsets": offsets, "known": known})
            }
            "store.funcs" => {
                let l = &self.layout;
                let total = a["total"].as_i64().unwrap_or(0);
                let (values, texts, atexts, assigns) = (strings(&a["values"]), strings(&a["texts"]), strings(&a["assign_texts"]), strings(&a["assigns"]));
                let groups: Vec<i64> = a["groups"].as_array().map(|g| g.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
                json!({
                    "parse_value": values.iter().map(|v| store::parse_value(v, total)).collect::<Vec<_>>(),
                    "parse_corr": values.iter().map(|v| store::parse_corr(v, total)).collect::<Vec<_>>(),
                    "text_hash": texts.iter().map(|t| store::text_hash(t)).collect::<Vec<_>>(),
                    "assign_from_text": atexts.iter().map(|t| store::assign_from_text(t)).collect::<Vec<_>>(),
                    "assign_to_text": assigns.iter().map(|v| store::assign_to_text(v)).collect::<Vec<_>>(),
                    "valid_assign": assigns.iter().map(|v| store::valid_assign(v, l)).collect::<Vec<_>>(),
                    "assign_label": assigns.iter().map(|v| store::assign_label(v, l).wire()).collect::<Vec<_>>(),
                    "group_label": groups.iter().map(|n| store::group_label(&n.to_string(), l).wire()).collect::<Vec<_>>(),
                    "ep_states": store::ep_states().iter().map(|(k, m)| json!([k, m.wire()])).collect::<Vec<_>>(),
                    "ep_of": assigns.iter().map(|v| store::ep_of(v)).collect::<Vec<_>>(),
                })
            }
            "events.clean_author" => json!(strings(&a["names"]).iter().map(|n| events::clean_author(n)).collect::<Vec<_>>()),
            "events.open" => {
                let log = EventLog::new(&self.p(&a["dir"]), &self.p(&a["seen"]), a["author"].as_str().unwrap_or(""));
                let answer = json!({"author": log.author, "known": log.known, "offsets": offsets_json(&log.offsets)});
                self.log = Some(log);
                answer
            }
            "events.append" => {
                let changes: Vec<(String, events::Key, Value)> = a["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| Some((c[0].as_str()?.to_string(), events::key_of(c[1].as_array()?)?, c[2].clone())))
                    .collect();
                let log = self.log.as_mut().ok_or("no event log")?;
                json!(log.append(&changes, clock).map_err(|e| e.to_string())?)
            }
            "events.replay_all" => {
                let log = self.log.as_mut().ok_or("no event log")?;
                let (seen, unseen, _) = log.read_all();
                log.state.clear();
                log.replay(&seen);
                log.replay(&unseen);
                json!([seen.len(), unseen.len()])
            }
            "events.dump" => {
                let log = self.log.as_ref().ok_or("no event log")?;
                let (seen, unseen, ends) = log.read_all();
                let pairs = |v: &[(events::Event, String)]| v.iter().map(|(e, n)| json!([e, n])).collect::<Vec<_>>();
                let state: Map<String, Value> = log
                    .state
                    .iter()
                    .map(|(kind, slot)| {
                        (kind.clone(), Value::Object(slot.iter().map(|(key, (v, st))| (py_dumps_key(key), json!([v, [st.0, st.1, st.2]]))).collect()))
                    })
                    .collect();
                let flat: Map<String, Value> = log
                    .flat()
                    .iter()
                    .map(|(kind, slot)| (kind.clone(), Value::Object(slot.iter().map(|(key, v)| (py_dumps_key(key), v.clone())).collect())))
                    .collect();
                json!({
                    "state": state, "flat": flat, "offsets": offsets_json(&log.offsets), "known": log.known,
                    "read_all": {"seen": pairs(&seen), "unseen": pairs(&unseen), "ends": offsets_json(&ends)},
                })
            }
            "write_file" => {
                let p = self.p(&a["path"]);
                std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
                let append = a["append"].as_bool().unwrap_or(false);
                let mut f = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&p).map_err(|e| e.to_string())?;
                std::io::Write::write_all(&mut f, a["text"].as_str().unwrap_or("").as_bytes()).map_err(|e| e.to_string())?;
                json!(true)
            }
            "set_local" => {
                self.clock.set_local(a["value"].as_str().unwrap_or("").parse().map_err(|e| format!("{e}"))?);
                json!(true)
            }
            "remove_file" => {
                std::fs::remove_file(self.p(&a["path"])).map_err(|e| e.to_string())?;
                json!(true)
            }
            "history.open" => {
                self.hist = Some(History::new(&self.p(&a["path"]), self.backup_dir(a).as_deref()));
                json!(true)
            }
            "history.record" => {
                let h = self.hist.as_mut().ok_or("no history")?;
                let f = |k: &str| a[k].as_f64().unwrap_or(0.0);
                let n = |k: &str| a[k].as_i64().unwrap_or(0);
                let p = history::Progress {
                    stamp: a["stamp"].as_str().unwrap_or("").to_string(),
                    t_done: n("t_done"),
                    total: n("total"),
                    tpct: f("tpct"),
                    c_done: n("c_done"),
                    cpct: f("cpct"),
                };
                json!(h.record(&p, clock).map_err(|e| err_text(&e))?)
            }
            "history.dump" => {
                let h = self.hist.as_ref().ok_or("no history")?;
                let ts = |t: &chrono::NaiveDateTime| t.format("%Y-%m-%d %H:%M:%S").to_string();
                json!({
                    "rows": h.rows(),
                    "points": h.points().iter().map(|(t, d, tot, c)| json!([ts(t), d, tot, c])).collect::<Vec<_>>(),
                    "chart": h.chart_points().iter().map(|(t, x, y)| json!([ts(t), x, y])).collect::<Vec<_>>(),
                })
            }
            "history.estimate" => {
                let points = self.points(a)?;
                history::estimate(&points, a["done_now"].as_i64().unwrap_or(0), a["total_now"].as_i64().unwrap_or(0), today(a)?).to_json()
            }
            "history.goals" => {
                let points = self.points(a)?;
                history::goals(&points, a["remaining_translate"].as_i64().unwrap_or(0), a["remaining_proof"].as_i64().unwrap_or(0), a.get("cfg"), today(a)?)
            }
            "texthistory.open" => {
                self.th = Some(TextHistory::new(&self.p(&a["path"])));
                json!(true)
            }
            "texthistory.load" => {
                let th = self.th.as_mut().ok_or("no text history")?;
                th.load_with(
                    a["max_lines"].as_u64().map_or(texthistory::MAX_LINES, |n| n as usize),
                    a["keep_lines"].as_u64().map_or(texthistory::KEEP_LINES, |n| n as usize),
                );
                dump_th(th)
            }
            "texthistory.record" => {
                let items: Vec<(String, (String, String), String, String)> = a["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| {
                        let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                        (s(&i["file"]), (s(&i["key"][0]), s(&i["key"][1])), s(&i["old"]), s(&i["new"]))
                    })
                    .collect();
                let th = self.th.as_mut().ok_or("no text history")?;
                let written = th.record(&items, a["via"].as_str().unwrap_or(""), clock);
                json!({"written": written, "state": dump_th(th)})
            }
            "texthistory.versions" => {
                let th = self.th.as_ref().ok_or("no text history")?;
                json!(th.versions(
                    a["file"].as_str().unwrap_or(""),
                    (a["key"][0].as_str().unwrap_or(""), a["key"][1].as_str().unwrap_or("")),
                    a["current"].as_str().unwrap_or("")
                ))
            }
            "texthistory.diff" => {
                let pack = lexo_core::lang::Pack::neutral();
                let parse = |k: &str| lexo_core::rpy::parse_text(a[k].as_str().unwrap_or(""), "", &pack).entries;
                json!(texthistory::diff_files(&parse("old"), &parse("new")).iter().map(|c| c.to_json()).collect::<Vec<_>>())
            }
            "memory.build" => {
                let mut rows: Vec<(String, i64, String, String, String)> = a["rows"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                        (s(&r[0]), r[1].as_i64().unwrap_or(0), s(&r[2]), s(&r[3]), s(&r[4]))
                    })
                    .collect();
                if let Some(g) = a.get("generate").filter(|g| g.is_object()) {
                    let s = |k: &str| g[k].as_str().unwrap_or("").to_string();
                    for i in 0..g["n"].as_u64().unwrap_or(0) as usize {
                        rows.push((s("file"), i as i64 + 1, s("who"), s("source").replace("{i}", &i.to_string()), s("target").replace("{i}", &i.to_string())));
                    }
                }
                let m = Memory::new(&rows);
                let answer = json!({"rows": m.rows.len(), "common": m.common});
                self.mem = Some(m);
                answer
            }
            "memory.exact" => {
                json!(self.mem.as_ref().ok_or("no memory")?.exact_matches(a["source"].as_str().unwrap_or(""), skip(a).as_ref().map(|(f, l)| (f.as_str(), *l))))
            }
            "memory.similar" => json!(self.mem.as_ref().ok_or("no memory")?.similar(
                a["source"].as_str().unwrap_or(""),
                skip(a).as_ref().map(|(f, l)| (f.as_str(), *l)),
                a["limit"].as_u64().unwrap_or(5) as usize,
                a["min_ratio"].as_f64().unwrap_or(0.7)
            )),
            "memory.groups" => json!(self.mem.as_ref().ok_or("no memory")?.groups(
                a["limit"].as_u64().unwrap_or(500) as usize,
                a["loose"].as_bool().unwrap_or(false),
                a["by_speaker"].as_bool().unwrap_or(false)
            )),
            "memory.tokens" => {
                let texts = strings(&a["texts"]);
                json!({
                    "tokens": texts.iter().map(|t| memory::tokens(t)).collect::<Vec<_>>(),
                    "tags": texts.iter().map(|t| memory::tags_of(t).into_iter().map(|(k, n)| (k, json!(n))).collect::<Map<String, Value>>()).collect::<Vec<_>>(),
                })
            }
            "snapshots.create" => snap(
                snapshots::create(&self.p(&a["root"]), &self.p(&a["data"]), a["reason"].as_str().unwrap_or("manual"), a["app"].as_str().unwrap_or(""), clock)
                    .map_err(|e| e.to_string())?,
            ),
            "snapshots.list" => json!(snapshots::list_snapshots(&self.p(&a["data"])).into_iter().map(|m| snap(Some(m))).collect::<Vec<_>>()),
            "snapshots.prune" => json!(snapshots::prune(&self.p(&a["data"]), a["keep"].as_i64().unwrap_or(0))),
            "snapshots.compare" => {
                let r = snapshots::compare(&self.p(&a["root"]), &self.p(&a["data"]), a["name"].as_str().unwrap_or("")).map_err(|e| e.to_string())?;
                json!(r.into_iter().map(|(f, s)| json!({"file": f, "state": s})).collect::<Vec<_>>())
            }
            "snapshots.restore" => {
                let files = a.get("files").filter(|f| f.is_array()).map(strings);
                let (restored, safety) = snapshots::restore(
                    &self.p(&a["root"]),
                    &self.p(&a["data"]),
                    a["name"].as_str().unwrap_or(""),
                    files.as_deref(),
                    a["app"].as_str().unwrap_or(""),
                    clock,
                )
                .map_err(|e| e.to_string())?;
                json!({"restored": restored, "safety": snap(safety)})
            }
            "snapshots.delete" => {
                snapshots::delete(&self.p(&a["data"]), a["name"].as_str().unwrap_or("")).map_err(|e| e.to_string())?;
                json!(true)
            }
            "snapshots.due" => json!(snapshots::due(&self.p(&a["data"]), a["interval_hours"].as_f64().unwrap_or(0.0), clock)),
            "snapshots.recent" => json!(snapshots::recent(&self.p(&a["data"]), a["minutes"].as_f64().unwrap_or(0.0), clock)),
            "snapshots.age" => json!(snapshots::last_age_hours(&self.p(&a["data"]), clock)),
            "snapshots.read" => json!(snapshots::digests(&self.p(&a["data"]), a["name"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|(n, h)| json!([n, h]))
                .collect::<Vec<_>>()),
            other => return Err(format!("unknown data step {other}")),
        })
    }
}

fn offsets_json(o: &indexmap::IndexMap<String, u64>) -> Value {
    Value::Object(o.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
}

/// `json.dumps(list(key))` of Python: `", "` between the elements, everything outside ASCII written as `\uXXXX`.
fn py_dumps_key(key: &events::Key) -> String {
    let items: Vec<String> = key
        .iter()
        .map(|k| {
            let compact = k.to_value().to_string();
            let mut out = String::new();
            for c in compact.chars() {
                if c.is_ascii() {
                    out.push(c);
                } else {
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf) {
                        out.push_str(&format!("\\u{u:04x}"));
                    }
                }
            }
            out
        })
        .collect();
    format!("[{}]", items.join(", "))
}

impl Runner<'_> {
    fn store(&mut self) -> Result<&mut Store, String> {
        self.store.as_mut().ok_or_else(|| "no store open".to_string())
    }

    fn change<F: FnOnce(&mut Store)>(&mut self, kind: Kind, f: F) -> Result<Value, String> {
        let clock: &dyn Clock = self.clock;
        self.store()?.change(kind.name(), &[kind], f, clock).map_err(|e| err_text(&e))?;
        self.dump_store()
    }

    fn dump_store(&mut self) -> Result<Value, String> {
        Ok(store_json(self.store()?))
    }
}

/// The state of a store in the shape of the golden data (also used to compare real data folders).
pub fn store_json(st: &Store) -> Value {
    {
        let corr: Map<String, Value> = st.corr.iter().map(|(k, v)| (k.clone(), json!({"c": v.c, "t": v.t}))).collect();
        let approved: Map<String, Value> =
            st.approved.iter().map(|(f, set)| (f.clone(), json!(set.iter().map(|(b, o)| json!([b, o])).collect::<Vec<_>>()))).collect();
        let proof: Map<String, Value> = st
            .proof
            .iter()
            .map(|(f, t)| {
                let mut items: Vec<_> = t.iter().collect();
                items.sort_by(|a, b| a.0.cmp(b.0));
                (f.clone(), json!(items.into_iter().map(|((b, o), m)| json!([b, o, m])).collect::<Vec<_>>()))
            })
            .collect();
        let ignored: Map<String, Value> = st
            .ignored
            .iter()
            .map(|(f, t)| {
                let mut items: Vec<_> = t.iter().collect();
                items.sort_by(|a, b| a.0.cmp(b.0));
                (f.clone(), json!(items.into_iter().map(|((b, o, c), m)| json!([b, o, c, m])).collect::<Vec<_>>()))
            })
            .collect();
        json!({
            "corr": corr, "assign": st.assign.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<String, Value>>(), "approved": approved,
            "episodes": st.episodes.iter().map(|(n, s)| (n.to_string(), json!(s))).collect::<Map<String, Value>>(),
            "proof": proof, "ignored": ignored,
            "glossary": st.glossary.iter().map(|(k, g)| (k.clone(), json!({"n": g.n, "t": g.t, "c": g.c}))).collect::<Map<String, Value>>(),
            "warnings": st.warnings.iter().map(|m| m.wire()).collect::<Vec<_>>(),
            "unsaved": st.unsaved.iter().collect::<Vec<_>>(), "undo": st.undo_labels(),
        })
    }
}

/// What the real-data comparison reads from a data folder (a `.lexo` folder): the store, the history and the events, nothing is written.
pub fn folder_json(dir: &Path, today: chrono::NaiveDate) -> Value {
    let layout = std::fs::read_to_string(dir.join("lexo.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| layout::parse(&v).ok())
        .unwrap_or_else(|| Layout::new(Vec::new(), Default::default(), "", "", Vec::new()));
    let mut st = Store::new(dir, None, layout);
    st.load();
    let h = History::new(&dir.join("history.csv"), None);
    let points = h.points();
    let ts = |t: &chrono::NaiveDateTime| t.format("%Y-%m-%d %H:%M:%S").to_string();
    let (done, total) = points.last().map_or((0, 0), |p| (p.1, p.2));
    let log = EventLog::new(&dir.join("events"), &dir.join("no-seen-file.json"), "nobody");
    let (seen, unseen, _) = log.read_all();
    let mut state = events::State::new();
    EventLog::replay_into(&mut state, &seen);
    EventLog::replay_into(&mut state, &unseen);
    let flat: Map<String, Value> = state
        .iter()
        .map(|(kind, slot)| {
            let items: Vec<Value> =
                slot.iter().filter(|(_, (v, _))| !v.is_null()).map(|(k, (v, s))| json!([events::key_json(k), v, [s.0, s.1, s.2]])).collect();
            (kind.clone(), Value::Array(items))
        })
        .collect();
    render(&json!({
        "store": store_json(&st),
        "history": {
            "rows": h.rows(),
            "points": points.iter().map(|(t, d, tot, c)| json!([ts(t), d, tot, c])).collect::<Vec<_>>(),
            "chart": h.chart_points().iter().map(|(t, x, y)| json!([ts(t), x, y])).collect::<Vec<_>>(),
            "estimate": history::estimate(&points, done, total, today).to_json(),
            "goals": history::goals(&points, (total - done).max(0), (total - done).max(0), None, today),
        },
        "events": flat,
    }))
}

fn today(a: &Value) -> Result<chrono::NaiveDate, String> {
    a["today"].as_str().unwrap_or("").parse().map_err(|e| format!("{e}"))
}

fn skip(a: &Value) -> Option<(String, i64)> {
    let s = a.get("skip").and_then(Value::as_array).filter(|s| s.len() == 2)?;
    Some((s[0].as_str()?.to_string(), s[1].as_i64()?))
}

/// A manifest without the size of the zip (the compressor decides it).
fn snap(m: Option<Map<String, Value>>) -> Value {
    match m {
        None => Value::Null,
        Some(mut m) => {
            m.shift_remove("size");
            Value::Object(m)
        }
    }
}

fn dump_th(th: &TextHistory) -> Value {
    let mut items: Vec<_> = th.by_key.iter().collect();
    items.sort_by(|a, b| a.0.cmp(b.0));
    json!({"total": th.total, "by_key": items.into_iter().map(|((f, b, s), recs)| json!([f, b, s, recs])).collect::<Vec<_>>()})
}

impl Runner<'_> {
    fn points(&self, a: &Value) -> Result<Vec<history::Point>, String> {
        if let Some(p) = a.get("points").and_then(Value::as_array) {
            return p
                .iter()
                .map(|r| {
                    let ts = chrono::NaiveDateTime::parse_from_str(r[0].as_str().unwrap_or(""), history::STAMP_FORMAT).map_err(|e| e.to_string())?;
                    Ok((ts, r[1].as_i64().unwrap_or(0), r[2].as_i64().unwrap_or(0), r[3].as_i64().unwrap_or(0)))
                })
                .collect();
        }
        Ok(self.hist.as_ref().ok_or("no history")?.points())
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
        let mut runner = Runner {
            root: &tmp,
            cfg_dir: tmp.join("cfg"),
            clock: &clock,
            locks: HashMap::new(),
            config: None,
            layout: Layout::new(Vec::new(), Default::default(), "", "", Vec::new()),
            store: None,
            log: None,
            hist: None,
            th: None,
            mem: None,
        };
        let mut steps = Vec::new();
        for st in spec["steps"].as_array().ok_or("no steps")? {
            let op = st["op"].as_str().unwrap_or("");
            let params: Map<String, Value> = st.as_object().unwrap().iter().filter(|(k, _)| *k != "op").map(|(k, v)| (k.clone(), v.clone())).collect();
            let mut entry = Map::new();
            entry.insert("op".into(), json!(op));
            entry.insert("params".into(), Value::Object(params));
            match runner.run(op, st) {
                Ok(v) => entry.insert("answer".into(), render(&v)),
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
