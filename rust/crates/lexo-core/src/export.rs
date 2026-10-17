//! Port of `core/lexo/export.py`: public progress data (`progress.json`, schema 2), the single source of truth for the website.
//!
//! Definitions (identical for the overall figure, seasons and episodes): a "line" is one translation unit; overall = every scanned file of the project, including shared
//! ('general') and unassigned files; an episode / season = only the files assigned to its episodes; overall = sum(episodes) + general + unassigned, which `validate_progress`
//! checks before writing. A planned season has no files, no figures and never enters any percentage. The file deliberately contains no file names or paths.
use crate::clock::Clock;
use crate::fsutil::{atomic_write_text, WriteFailed};
use crate::history::{self, History};
use crate::layout::Layout;
use crate::messages::{msg, Message};
use crate::project::{file_status, totals_of, Row, STATUS_ORDER};
use crate::pyfmt::{json_dumps, round_digits};
use crate::store::{ep_of, Store};
use chrono::{NaiveDate, NaiveDateTime};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const SCHEMA: i64 = 2;

pub fn pct(done: i64, total: i64) -> Value {
    if total != 0 {
        json!(round_digits(done as f64 / total as f64 * 100.0, 1))
    } else {
        Value::Null
    }
}

pub fn block(done: i64, total: i64) -> Value {
    json!({"done": done, "total": total, "percent": pct(done, total)})
}

/// (translated, total, proofread, files, files done, files proofread)
fn accumulate(rows: &[&Row]) -> (i64, i64, i64, i64, i64, i64) {
    (
        rows.iter().map(|r| r.done).sum(),
        rows.iter().map(|r| r.total).sum(),
        rows.iter().map(|r| r.corr).sum(),
        rows.len() as i64,
        rows.iter().filter(|r| r.todo == 0).count() as i64,
        rows.iter().filter(|r| r.corr >= r.total).count() as i64,
    )
}

/// Structured statistics used by both the exporter and the manager's overview.
pub fn stats_for(rows: &[Row], store: &Store, layout: &Layout) -> Value {
    let active = layout.active_groups();
    let mut eps: BTreeMap<i64, Vec<&Row>> = active.iter().map(|n| (*n, Vec::new())).collect();
    let (mut general, mut unassigned): (Vec<&Row>, Vec<&Row>) = (Vec::new(), Vec::new());
    for r in rows {
        let v = store.assign.get(&r.name).map_or("", String::as_str);
        match ep_of(v).filter(|n| eps.contains_key(n)) {
            Some(n) => eps.get_mut(&n).unwrap().push(r),
            None if v == "gen" => general.push(r),
            None => unassigned.push(r),
        }
    }
    let mut episodes: Vec<Map<String, Value>> = Vec::new();
    for n in &active {
        let (d, t, c, nf, nf_done, nf_proof) = accumulate(&eps[n]);
        let m = json!({
            "n": n, "season": layout.section_of(*n), "status": store.episodes.get(n).map_or("none", String::as_str), "translation": block(d, t), "proofreading": block(c, t),
            "files": nf, "files_done": nf_done, "files_proofread": nf_proof,
        });
        episodes.push(m.as_object().unwrap().clone());
    }
    let mut seasons: Vec<Value> = Vec::new();
    for s in &layout.sections {
        if s.status != "active" {
            seasons.push(json!({"n": s.n, "status": s.status}));
            continue;
        }
        let mine: Vec<&Map<String, Value>> = episodes.iter().filter(|e| e["season"] == json!(s.n)).collect();
        let sum = |key: &str, f: &str| mine.iter().map(|e| e[key][f].as_i64().unwrap_or(0)).sum::<i64>();
        let (d, t, c) = (sum("translation", "done"), sum("translation", "total"), sum("proofreading", "done"));
        seasons.push(json!({
            "n": s.n, "status": "active", "episodes": s.groups, "translation": block(d, t), "proofreading": block(c, t),
            "files": mine.iter().map(|e| e["files"].as_i64().unwrap_or(0)).sum::<i64>(),
            "files_done": mine.iter().map(|e| e["files_done"].as_i64().unwrap_or(0)).sum::<i64>(),
            "episodes_done": mine.iter().filter(|e| e["status"] == "done").count(), "episodes_total": mine.len(),
        }));
    }
    let part = |rs: &[&Row]| {
        let (d, t, c, nf, _, _) = accumulate(rs);
        json!({"translation": block(d, t), "proofreading": block(c, t), "files": nf})
    };
    json!({"episodes": episodes, "seasons": seasons, "general": part(&general), "unassigned": part(&unassigned)})
}

/// The local time with its offset as `datetime.now().astimezone().isoformat(timespec="minutes")` writes it.
pub fn now_minutes(clock: &dyn Clock) -> String {
    let (local, utc): (NaiveDateTime, NaiveDateTime) = (clock.now_local(), clock.now_utc().naive_utc());
    let offset = (local - utc).num_minutes();
    let (sign, abs) = if offset < 0 { ('-', -offset) } else { ('+', offset) };
    format!("{}{sign}{:02}:{:02}", local.format("%Y-%m-%dT%H:%M"), abs / 60, abs % 60)
}

/// The public `progress.json` as JSON. `now` is the "updated" text (see `now_minutes`).
pub fn build_progress(rows: &[Row], store: &Store, layout: &Layout, history: &History, title: &str, now: &str, today: NaiveDate) -> Value {
    let (t_done, total, c_done, _tpct, _cpct) = totals_of(rows);
    let mut counts: Map<String, Value> = STATUS_ORDER.iter().map(|k| (k.to_string(), json!(0))).collect();
    for r in rows {
        let k = file_status(r.done, r.todo, r.total, r.corr);
        let n = counts[k].as_i64().unwrap_or(0) + 1;
        counts.insert(k.to_string(), json!(n));
    }
    let st = stats_for(rows, store, layout);

    let mut by_day: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for (ts, t, c) in history.chart_points() {
        by_day.insert(ts.format("%Y-%m-%d").to_string(), (t, c)); // last record of each day
    }
    let all: Vec<Value> = by_day.iter().map(|(d, v)| json!({"date": d, "translation": v.0, "proofreading": v.1})).collect();
    let chart: Vec<Value> = all[all.len().saturating_sub(180)..].to_vec();

    let est = history::estimate(&history.points(), t_done, total, today).to_json();
    let estimate: Map<String, Value> = ["status", "remaining", "pace_per_day", "eta", "eta_early", "eta_late", "confidence", "basis_days", "days_with_data"]
        .iter()
        .map(|k| (k.to_string(), est[*k].clone()))
        .collect();

    let mut episodes: Vec<Value> = Vec::new();
    for e in st["episodes"].as_array().unwrap() {
        // schema 1 keys first, then the schema 2 additions
        let mut item = Map::new();
        item.insert("n".into(), e["n"].clone());
        item.insert("status".into(), e["status"].clone());
        if e["files"].as_i64().unwrap_or(0) != 0 {
            item.insert("percent".into(), e["translation"]["percent"].clone());
            item.insert("files".into(), e["files"].clone());
        }
        item.insert("season".into(), e["season"].clone());
        item.insert("translation".into(), e["translation"].clone());
        item.insert("proofreading".into(), e["proofreading"].clone());
        item.insert("files_done".into(), e["files_done"].clone());
        item.insert("files_total".into(), e["files"].clone());
        item.insert("files_proofread".into(), e["files_proofread"].clone());
        episodes.push(Value::Object(item));
    }
    let mut files = counts;
    files.insert("total".into(), json!(rows.len()));
    let seasons_of = |status: &str| layout.sections.iter().filter(|s| s.status == status).map(|s| s.n).collect::<Vec<_>>();
    json!({
        "schema": SCHEMA, "title": title, "updated": now, "translation": block(t_done, total), "proofreading": block(c_done, total), "files": files,
        "scope": {"seasons": seasons_of("active"), "episodes": layout.active_groups(), "planned_seasons": seasons_of("planned")},
        "seasons": st["seasons"], "episodes": episodes, "general": st["general"], "unassigned": st["unassigned"], "estimate": estimate, "history": chart,
    })
}

fn plain(text: String) -> Message {
    Message::new(&text, None, Map::new())
}

fn is_int(v: &Value) -> bool {
    v.as_i64().is_some() || v.as_u64().is_some()
}

/// The problems of the data (empty = safe to write / publish).
pub fn validate_progress(data: &Value, layout: &Layout) -> Vec<Message> {
    let mut p: Vec<Message> = Vec::new();
    let Some(obj) = data.as_object() else { return vec![plain("Data nejsou objekt.".into())] };
    for key in ["translation", "proofreading"] {
        let b = obj.get(key).and_then(Value::as_object).filter(|b| ["done", "total", "percent"].iter().all(|k| b.contains_key(*k)));
        let Some(b) = b else {
            p.push(msg("The block “{key}” is missing.", &[("key", json!(key))]));
            continue;
        };
        let (done, total) = (&b["done"], &b["total"]);
        let bad_values = !(is_int(done) && is_int(total)) || done.as_i64().unwrap_or(0) < 0 || done.as_i64() > total.as_i64();
        if bad_values {
            p.push(msg("{key}: invalid done/total values ({done}/{total}).", &[("key", json!(key)), ("done", done.clone()), ("total", total.clone())]));
        }
        if let Some(pc) = b["percent"].as_f64() {
            if !(0.0..=100.0).contains(&pc) {
                p.push(plain(format!("{key}: procenta mimo rozsah ({}).", crate::messages::py_str(&b["percent"]))));
            }
        }
    }
    let (Some(eps), Some(seasons)) = (obj.get("episodes").and_then(Value::as_array), obj.get("seasons").and_then(Value::as_array)) else {
        p.push(msg("The list of episodes or seasons is missing.", &[]));
        return p;
    };
    let active = layout.active_groups();
    let nums: Vec<Value> = eps.iter().map(|e| e.get("n").cloned().unwrap_or(Value::Null)).collect();
    if nums != active.iter().map(|n| json!(n)).collect::<Vec<_>>() {
        p.push(msg(
            "The episodes {nums} do not match the active range {active}.",
            &[("nums", json!(py_list(&nums))), ("active", json!(py_list(&active.iter().map(|n| json!(n)).collect::<Vec<_>>())))],
        ));
    }
    for e in eps {
        let n = e.get("n").and_then(Value::as_i64);
        let expected = n.and_then(|n| layout.section_of(n));
        let season = e.get("season").and_then(Value::as_i64);
        if season != expected {
            p.push(msg(
                "Episode {n} is in season {season}, correct is {expected}.",
                &[
                    ("n", e.get("n").cloned().unwrap_or(Value::Null)),
                    ("season", e.get("season").cloned().unwrap_or(Value::Null)),
                    ("expected", json!(expected)),
                ],
            ));
        }
        let t = e.get("translation").cloned().unwrap_or(json!({}));
        if t.get("done").and_then(Value::as_i64).unwrap_or(0) > t.get("total").and_then(Value::as_i64).unwrap_or(0) {
            p.push(msg("Episode {n}: more translated than the total.", &[("n", e.get("n").cloned().unwrap_or(Value::Null))]));
        }
    }
    let by_n: BTreeMap<i64, &Value> = seasons.iter().filter_map(|s| Some((s.get("n")?.as_i64()?, s))).collect();
    for s in &layout.sections {
        match by_n.get(&s.n) {
            None => p.push(msg("Season {n} is missing.", &[("n", json!(s.n))])),
            Some(got) if got.get("status").and_then(Value::as_str) != Some(s.status.as_str()) => p.push(msg(
                "Season {n} has the state {got}, correct is {expected}.",
                &[("n", json!(s.n)), ("got", got.get("status").cloned().unwrap_or(Value::Null)), ("expected", json!(s.status))],
            )),
            Some(got) if s.status == "planned" && got.as_object().is_some_and(|o| o.keys().any(|k| k != "n" && k != "status")) => {
                p.push(msg("The planned season {n} must not have any numbers.", &[("n", json!(s.n))]));
            }
            _ => {}
        }
    }
    let tr = |e: &Value, f: &str| e["translation"][f].as_i64().unwrap_or(0);
    for s in seasons {
        if s.get("status").and_then(Value::as_str) == Some("active") {
            let mine: Vec<&Value> = eps.iter().filter(|e| e.get("season") == s.get("n")).collect();
            let (d, t) = (mine.iter().map(|e| tr(e, "done")).sum::<i64>(), mine.iter().map(|e| tr(e, "total")).sum::<i64>());
            if d != tr(s, "done") || t != tr(s, "total") {
                p.push(msg("Season {n}: the sum of the episodes does not match the season.", &[("n", s.get("n").cloned().unwrap_or(Value::Null))]));
            }
        }
    }
    let part = |k: &str, f: &str| obj.get(k).and_then(|b| b.get("translation")).and_then(|t| t.get(f)).and_then(Value::as_i64);
    match (part("general", "done"), part("unassigned", "done"), part("general", "total"), part("unassigned", "total")) {
        (Some(gd), Some(ud), Some(gt), Some(ut)) => {
            let (parts_done, parts_total) =
                (eps.iter().map(|e| tr(e, "done")).sum::<i64>() + gd + ud, eps.iter().map(|e| tr(e, "total")).sum::<i64>() + gt + ut);
            let (c, d) = (obj["translation"]["done"].as_i64().unwrap_or(0), obj["translation"]["total"].as_i64().unwrap_or(0));
            if (parts_done, parts_total) != (c, d) {
                p.push(msg(
                    "The sum of episodes + general + unassigned ({a}/{b}) does not match the total ({c}/{d}).",
                    &[("a", json!(parts_done)), ("b", json!(parts_total)), ("c", json!(c)), ("d", json!(d))],
                ));
            }
        }
        _ => p.push(msg("The blocks general / unassigned are missing.", &[])),
    }
    let text = json_dumps(data, false);
    if Regex::new(r"\.rpy\b").unwrap().is_match(&text) {
        p.push(msg("Public data must not contain file names (.rpy).", &[]));
    }
    p
}

/// A Python list printed as `[1, 2]`.
fn py_list(v: &[Value]) -> String {
    format!("[{}]", v.iter().map(crate::messages::py_str).collect::<Vec<_>>().join(", "))
}

/// `json.dumps(data, ensure_ascii=False, indent=1) + "\n"`
pub fn render(data: &Value) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b" "));
    let _ = data.serialize(&mut ser);
    String::from_utf8_lossy(&buf).into_owned() + "\n"
}

#[derive(Debug, PartialEq)]
pub enum ExportError {
    /// The data did not pass the check.
    Invalid(Message),
    Write(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Invalid(m) => f.write_str(&m.text()),
            ExportError::Write(s) => f.write_str(s),
        }
    }
}

/// Validate and write `progress.json` atomically.
pub fn write_local(path: &Path, data: &Value, layout: &Layout, backup_dir: Option<&Path>, clock: &dyn Clock) -> Result<(), ExportError> {
    let problems = validate_progress(data, layout);
    if !problems.is_empty() {
        let joined = problems.iter().map(Message::text).collect::<Vec<_>>().join("; ");
        return Err(ExportError::Invalid(msg("The export did not pass the check: {problems}", &[("problems", json!(joined))])));
    }
    atomic_write_text(path, &render(data), Some("\n"), backup_dir, clock).map_err(|e: WriteFailed| ExportError::Write(e.to_string()))
}

/// Validate the target path of the progress file inside the website repository (relative, no `..`, a `.json` file).
pub fn clean_target(rel: &str) -> Result<String, Message> {
    let rel = crate::util::py_strip(&rel.replace('\\', "/")).to_string();
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    let absolute = rel.starts_with('/');
    let drive = rel.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && rel.chars().nth(1) == Some(':');
    let last = parts.last().copied().unwrap_or("");
    let suffix = match last.rfind('.') {
        Some(i) if i > 0 && i < last.len() - 1 => last[i..].to_lowercase(),
        _ => String::new(),
    };
    if rel.is_empty() || absolute || parts.contains(&"..") || drive || suffix != ".json" {
        return Err(msg("Invalid path of the file in the repository: {rel} (it must be a relative path to a .json file).", &[("rel", json!(rel))]));
    }
    Ok(parts.join("/"))
}
