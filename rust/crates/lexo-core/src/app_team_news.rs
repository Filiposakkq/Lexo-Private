//! Team work, the second part: what each person did (activity log and statistics for the lead), notes to the team on lines, and the news since your last look. More files, each written by one
//! person only (see `app_team.rs` for the first two):
//!
//! * `.aside/team/activity/<author>.jsonl` – what this person changed in the texts: one line per file and edit `{t, a, file, n, via}` (written only in a project that has a team);
//! * `.aside/team/notes/<author>.jsonl` – notes to the team on lines `{id, t, a, file, k: [block, original], text}` and resolutions `{resolve: id, t, a}`;
//! * `<local data>/team-seen.json` (not shared) – the time of your last look at the news.
//!
//! Statistics are the Pro feature `team.manage` and only for the lead; notes and news are for every member (Free). These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::app_team::{read_json, FEATURE};
use crate::messages::{msg, py_str};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const TIME_FMT: &str = "%Y-%m-%dT%H:%M:%S%.6f+00:00";
const MAX_NOTE: usize = 2000;
const MAX_NEWS: usize = 40;

/// The lines of all `*.jsonl` files of a folder as JSON objects (a broken line is skipped).
fn read_lines(dir: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            out.extend(text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(Value::is_object));
        }
    }
    out
}

impl App {
    fn team_now(&self) -> String {
        self.clock.now_utc().format(TIME_FMT).to_string()
    }

    /// The time `days` days ago in the format of the logs (empty = from the beginning).
    fn team_since(&self, days: Option<i64>) -> String {
        days.filter(|d| *d > 0).map(|d| (self.clock.now_utc() - chrono::Duration::days(d)).format(TIME_FMT).to_string()).unwrap_or_default()
    }

    fn team_exists(&self) -> bool {
        self.team_dir().is_ok_and(|d| d.join("lead.json").is_file())
    }

    /// Called after every change of texts (`log_text`): in a project with a team it is added to the activity log of this person. A problem here never breaks an edit.
    pub fn team_activity(&mut self, details: &[Value], via: &str) {
        if details.is_empty() || !self.team_exists() {
            return;
        }
        let (me, now) = (self.author_id(), self.team_now());
        let mut per_file: BTreeMap<String, i64> = BTreeMap::new();
        for d in details {
            *per_file.entry(py_str(&d["file"])).or_insert(0) += 1;
        }
        for (file, n) in per_file {
            let _ = self.team_append_line("activity", &me, &json!({"t": now, "a": me, "file": file, "n": n, "via": via}));
        }
    }

    fn team_append_line(&self, folder: &str, author: &str, v: &Value) -> Res<()> {
        let dir = self.team_dir()?.join(folder);
        let io = |e: std::io::Error| app_err(msg("The team data could not be saved: {reason}", &[("reason", json!(e.to_string()))]));
        std::fs::create_dir_all(&dir).map_err(io)?;
        use std::io::Write;
        let path = dir.join(format!("{}.jsonl", crate::events::clean_author(author)));
        let mut f = std::fs::OpenOptions::new().append(true).create(true).open(path).map_err(io)?;
        f.write_all((v.to_string() + "\n").as_bytes()).map_err(io)
    }

    /// Statistics per person (Pro, the lead): the progress of their files, what they changed, approved and marked in the last `days` days (all time when none), their last activity.
    pub fn team_stats(&mut self, days: Option<i64>) -> Res<Value> {
        self.need_project()?;
        self.gate(FEATURE)?;
        let me = self.author_id();
        let Some(lead) = self.team_lead_file()? else { return Err(app_err(msg("This project has no team yet.", &[]))) };
        if py_str(&lead["lead"]) != me {
            return Err(app_err(msg("Only the lead of the team can see the statistics.", &[])));
        }
        let since = self.team_since(days);
        let team_dir = self.team_dir()?;
        let (events, rows) = {
            let o = self.o()?;
            let rows: Map<String, Value> =
                o.project.rows(&o.store).into_iter().map(|r| (r.name.clone(), json!({"total": r.total, "done": r.done, "corr": r.corr}))).collect();
            (read_lines(&o.data_dir.join("events")), rows)
        };
        let mut counts: BTreeMap<String, BTreeMap<&str, i64>> = BTreeMap::new();
        let mut last: BTreeMap<String, String> = BTreeMap::new();
        let mut bump = |a: &str, what: &'static str, n: i64, t: &str| {
            if since.is_empty() || t >= since.as_str() {
                *counts.entry(a.to_string()).or_default().entry(what).or_insert(0) += n;
            }
            let slot = last.entry(a.to_string()).or_default();
            if t > slot.as_str() {
                *slot = t.to_string();
            }
        };
        for ev in &events {
            let (a, t) = (py_str(&ev["a"]), py_str(&ev["t"]));
            if ev["value"].is_null() {
                continue;
            }
            match py_str(&ev["kind"]).as_str() {
                "approved" => bump(&a, "approved", 1, &t),
                "proof" => bump(&a, "marked", 1, &t),
                "corr" => bump(&a, "proofread_updates", 1, &t),
                _ => {}
            }
        }
        for ev in read_lines(&team_dir.join("activity")) {
            let via = py_str(&ev["via"]);
            if via != "undo" && via != "restore" {
                bump(&py_str(&ev["a"]), "edited", ev["n"].as_i64().unwrap_or(0), &py_str(&ev["t"]));
            }
        }
        let counts: Map<String, Value> = counts.into_iter().map(|(a, m)| (a, json!(m))).collect();
        let state = self.team_state()?;
        let res =
            self.ext_call(FEATURE, "stats", &json!({"members": state["members"], "assign": state["assign"], "rows": rows, "counts": counts, "last": last}))?;
        Ok(json!({"ok": true, "members": res["members"], "days": days, "unassigned": res["unassigned"]}))
    }

    fn team_name(&self, author: &str) -> String {
        self.team_members().ok().and_then(|m| m.get(author).cloned()).filter(|n| !n.is_empty()).unwrap_or_else(|| author.to_string())
    }

    /// The key of a line in the notes: its block with the original (the same as the line history uses).
    fn team_line_key(&self, rel: &str, line: i64) -> Res<(String, String)> {
        let (_, ent) = self.find_entry(rel, line).ok_or_else(crate::app::line_gone)?;
        Ok(ent.key())
    }

    /// A note to the team on a line (Free, in a project that has a team).
    pub fn team_note_add(&mut self, rel: &str, line: i64, text: &str) -> Res<Value> {
        self.need_project()?;
        if !self.team_exists() {
            return Err(app_err(msg("This project has no team yet.", &[])));
        }
        let text: String = crate::util::py_strip(text).chars().take(MAX_NOTE).collect();
        if text.is_empty() {
            return Err(app_err(msg("Write the note first.", &[])));
        }
        let key = self.team_line_key(rel, line)?;
        let (me, now) = (self.author_id(), self.team_now());
        let entry = json!({"id": format!("{now}|{me}"), "t": now, "a": me, "file": rel, "k": [key.0, key.1], "text": text});
        self.team_append_line("notes", &me, &entry)?;
        self.team_notes(rel, line)
    }

    /// Mark a note as dealt with (anybody in the team; it is a line of your own file).
    pub fn team_note_resolve(&mut self, id: &str, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let (me, now) = (self.author_id(), self.team_now());
        self.team_append_line("notes", &me, &json!({"resolve": id, "t": now, "a": me}))?;
        self.team_notes(rel, line)
    }

    /// The notes of a line, oldest first: `{exists, notes: [{id, author, name, t, text, resolved}]}`.
    pub fn team_notes(&mut self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        if !self.team_exists() {
            return Ok(json!({"ok": true, "exists": false, "notes": []}));
        }
        let key = self.team_line_key(rel, line)?;
        let all = read_lines(&self.team_dir()?.join("notes"));
        let resolved: BTreeSet<String> = all.iter().filter_map(|e| e.get("resolve").map(py_str)).collect();
        let mut notes: Vec<Value> = all
            .iter()
            .filter(|e| e.get("id").is_some() && py_str(&e["file"]) == rel && py_str(&e["k"][0]) == key.0 && py_str(&e["k"][1]) == key.1)
            .map(|e| json!({"id": e["id"], "author": e["a"], "name": self.team_name(&py_str(&e["a"])), "t": e["t"], "text": e["text"], "resolved": resolved.contains(&py_str(&e["id"]))}))
            .collect();
        notes.sort_by_key(|n| py_str(&n["t"]));
        Ok(json!({"ok": true, "exists": true, "notes": notes}))
    }

    fn team_seen_path(&self) -> Res<PathBuf> {
        Ok(self.o()?.local_dir.join("team-seen.json"))
    }

    /// The time of the last look at the news; the first time it is now (a new member does not get the whole history as news).
    fn team_seen(&mut self) -> Res<String> {
        let path = self.team_seen_path()?;
        if let Some(v) = read_json(&path) {
            let t = py_str(&v["t"]);
            if !t.is_empty() {
                return Ok(t);
            }
        }
        let now = self.team_now();
        self.team_write(&path, &json!({"t": now}))?;
        Ok(now)
    }

    /// What the others did since your last look: `{since, items: [{author, name, what, file, n}], notes, total}` (newest first, at most 40 lines).
    pub fn team_changes(&mut self) -> Res<Value> {
        self.need_project()?;
        if !self.team_exists() {
            return Ok(json!({"ok": true, "exists": false, "items": [], "notes": 0, "total": 0}));
        }
        let (me, since) = (self.author_id(), self.team_seen()?);
        let events = read_lines(&self.o()?.data_dir.join("events"));
        let team_dir = self.team_dir()?;
        let mut groups: BTreeMap<(String, &str, String), (i64, String)> = BTreeMap::new();
        let mut add = |a: &str, what: &'static str, file: String, n: i64, t: &str| {
            let slot = groups.entry((a.to_string(), what, file)).or_insert((0, String::new()));
            slot.0 += n;
            if t > slot.1.as_str() {
                slot.1 = t.to_string();
            }
        };
        for ev in &events {
            let (a, t) = (py_str(&ev["a"]), py_str(&ev["t"]));
            if a == me || t.as_str() <= since.as_str() || ev["value"].is_null() {
                continue;
            }
            let file = py_str(&ev["key"][0]);
            match py_str(&ev["kind"]).as_str() {
                "approved" => add(&a, "approved", file, 1, &t),
                "proof" => add(&a, "marked", file, 1, &t),
                "assign" => add(&a, "grouped", file, 1, &t),
                _ => {}
            }
        }
        for ev in read_lines(&team_dir.join("activity")) {
            let (a, t, via) = (py_str(&ev["a"]), py_str(&ev["t"]), py_str(&ev["via"]));
            if a != me && t.as_str() > since.as_str() && via != "undo" && via != "restore" {
                add(&a, "edited", py_str(&ev["file"]), ev["n"].as_i64().unwrap_or(0), &t);
            }
        }
        let notes = read_lines(&team_dir.join("notes"))
            .iter()
            .filter(|e| e.get("id").is_some() && py_str(&e["a"]) != me && py_str(&e["t"]).as_str() > since.as_str())
            .count();
        let mut items: Vec<(String, Value)> = groups
            .into_iter()
            .map(|((a, what, file), (n, t))| (t, json!({"author": a, "name": self.team_name(&a), "what": what, "file": file, "n": n})))
            .collect();
        items.sort_by(|x, y| y.0.cmp(&x.0));
        let total: i64 = items.iter().map(|(_, v)| v["n"].as_i64().unwrap_or(0)).sum::<i64>() + notes as i64;
        items.truncate(MAX_NEWS);
        Ok(json!({"ok": true, "exists": true, "since": since, "items": items.into_iter().map(|x| x.1).collect::<Vec<_>>(), "notes": notes, "total": total}))
    }

    /// "I have seen it": the news count from now.
    pub fn team_mark_seen(&mut self) -> Res<Value> {
        self.need_project()?;
        let path = self.team_seen_path()?;
        let now = self.team_now();
        self.team_write(&path, &json!({"t": now}))?;
        self.team_changes()
    }
}
