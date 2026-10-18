//! Port of `core/lexo/app_core.py`: the open project, refresh, persistence of derived files, settings, features, links.
use crate::api::{ok, plain, Answer, Args};
use crate::app::{App, CachedIssues, Open};
use crate::app_base::{app_err, ApiError, Res, LINKS, UI_PREF_MAX};
use crate::datadir;
use crate::events::{self, EventLog};
use crate::features;
use crate::fsutil::{resolve, write_csv};
use crate::history::{self, History, Progress};
use crate::layout;
use crate::lock::{Owner, ProjectLock};
use crate::messages::{msg, msgn, py_str, Message, Text};
use crate::project::{check_project_dir, find_translation_folder, totals_of, Project, DATA_DIR_NAME};
use crate::pyfmt::json_dumps;
use crate::store::{Kind, Store};
use crate::texthistory::{self, TextHistory};
use crate::{export, mt};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::Path;

impl App {
    // ------------------------------------------------------------ project lifecycle
    /// Release the project lock (called on exit).
    pub fn close(&mut self) {
        if let Some(o) = self.open.as_mut() {
            if let Some(mut l) = o.plock.take() {
                l.release();
            }
        }
    }

    pub fn open_project(&mut self, path: &str, force: bool) -> Res<Value> {
        let (ok_, problem) = check_project_dir(path);
        if !ok_ {
            return Err(app_err(problem.unwrap_or_else(|| msg("The folder cannot be used.", &[]))));
        }
        let (path, redirected) = find_translation_folder(Path::new(path)).map_err(app_err)?;
        if let Some(note) = redirected {
            self.notices.push(Text::new_msg(note));
        }
        let root = resolve(&path);
        crate::datadir::adopt_legacy(&root);
        let data_dir = root.join(DATA_DIR_NAME);
        if let Err(e) = std::fs::create_dir(&data_dir) {
            if e.kind() != std::io::ErrorKind::AlreadyExists || !data_dir.is_dir() {
                return Err(app_err(msg(
                    "Cannot create {folder} in the project folder ({reason}).",
                    &[("folder", json!(DATA_DIR_NAME)), ("reason", json!(e.to_string()))],
                )));
            }
        }
        let local_dir = datadir::local_dir(&data_dir, &root, &self.cfg_dir, &*self.clock);
        let mut new_lock = ProjectLock::new(&local_dir, Owner::this_process());
        if let Err(e) = new_lock.acquire(force, &*self.clock) {
            return Err(ApiError::Busy { text: Text::new_msg(e.message), path: root.to_string_lossy().into_owned() });
        }
        if let Some(o) = self.open.as_mut() {
            if let Some(mut old) = o.plock.take() {
                old.release();
            }
        }
        self.notices.retain(|n| !n.text().is_empty());
        let (lay, lay_note) = layout::load(&data_dir, &self.packs);
        if let Some(note) = lay_note {
            self.notices.push(Text::Plain(note));
        }
        let pack = self.packs.get(&lay.language).unwrap_or_else(crate::lang::Pack::neutral);
        let mut store = Store::new(&data_dir, Some(&local_dir), lay);
        store.load();
        let author = self.author_id();
        store.attach_events(EventLog::new(&data_dir.join("events"), &local_dir.join(events::SEEN_FILE), &author));
        let merged = store.sync_events(&*self.clock);
        if merged > 0 {
            self.notices.push(Text::new_msg(msgn(merged as i64, "{n} change by another author was merged.", "{n} changes by other authors were merged.", &[])));
        }
        self.notices.extend(store.warnings.iter().cloned().map(Text::new_msg));
        let skip: Vec<String> = self.config.get("skip_files").and_then(Value::as_array).map(|a| a.iter().map(py_str).collect()).unwrap_or_default();
        let symbols = self.config.get("count_symbol_only_as_done").and_then(Value::as_bool);
        let mut project = Project::new(&root, &skip, symbols, &pack);
        let history = History::new(&data_dir.join("history.csv"), Some(&local_dir.join("backups")));
        let mut texthist = TextHistory::new(&local_dir.join(texthistory::FILE_NAME));
        texthist.load();
        let report = project.refresh(true, &*self.clock).map_err(|e| app_err(e.0))?;
        let names: Vec<String> = project.rows(&store).into_iter().map(|r| r.name).collect();
        store.ensure_files(&names, &*self.clock);
        self.config.set("project_dir", json!(root.to_string_lossy()));
        if let Err(e) = self.config.save(&*self.clock) {
            self.notices.push(Text::new_msg(msg("The settings could not be saved: {reason}", &[("reason", e.reason.wire())])));
        }
        self.open = Some(Open {
            project,
            store,
            history,
            data_dir,
            local_dir,
            texthist,
            plock: Some(new_lock),
            val_cache: HashMap::new(),
            spell_cache: None,
            pack,
            gi: None,
            mem_cache: None,
        });
        self.persist()?;
        Ok(report.to_json())
    }

    /// Who this computer's changes are written as in the event log; made once from the user name and kept in the settings.
    pub fn author_id(&mut self) -> String {
        let current = self.config.get("author").map(py_str).unwrap_or_default();
        if !current.is_empty() {
            return current;
        }
        let name = ["LOGNAME", "USER", "LNAME", "USERNAME"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .unwrap_or_else(|| "author".to_string());
        let author = format!("{}-{}", events::clean_author(&name), crate::fsutil::token_hex(2));
        self.config.set("author", json!(author));
        author
    }

    // ------------------------------------------------------------ persistence of derived files
    /// Write progress.csv, the history row and the local progress.json. Returns a list of problems.
    pub fn persist(&mut self) -> Res<Vec<String>> {
        let mut problems: Vec<String> = Vec::new();
        let title = self.config.get("web.title").map(py_str).unwrap_or_default();
        let clock = &*self.clock;
        let o = self.open.as_mut().ok_or_else(|| app_err(msg("No project is open.", &[])))?;
        let rows = o.project.rows(&o.store);
        let mut table: Vec<Vec<String>> =
            vec![["file", "translated", "remaining", "total", "proofread", "translation_%", "proofreading_%"].iter().map(|s| s.to_string()).collect()];
        for r in &rows {
            table.push(vec![
                r.name.clone(),
                r.done.to_string(),
                r.todo.to_string(),
                r.total.to_string(),
                r.corr.to_string(),
                format!("{:.1}", r.done as f64 / r.total as f64 * 100.0),
                format!("{:.1}", r.corr as f64 / r.total as f64 * 100.0),
            ]);
        }
        if let Err(e) = write_csv(&o.local_dir.join("progress.csv"), &table, None, clock) {
            problems.push(e.to_string());
        }
        let (t_done, total, c_done, tpct, cpct) = totals_of(&rows);
        let progress = Progress { stamp: clock.now_local().format(history::STAMP_FORMAT).to_string(), t_done, total, tpct, c_done, cpct };
        if let Err(e) = o.history.record(&progress, clock) {
            problems.push(e.to_string());
        }
        let data = export::build_progress(&rows, &o.store, &o.store.layout, &o.history, &title, &export::now_minutes(clock), clock.now_local().date());
        if let Err(e) = export::write_local(&o.local_dir.join("progress.json"), &data, &o.store.layout, None, clock) {
            problems.push(e.to_string());
        }
        self.last_save_error = problems.join("; ");
        Ok(problems)
    }

    pub fn build_export(&self) -> Res<Value> {
        let o = self.o()?;
        let title = self.config.get("web.title").map(py_str).unwrap_or_default();
        let rows = o.project.rows(&o.store);
        Ok(export::build_progress(&rows, &o.store, &o.store.layout, &o.history, &title, &export::now_minutes(&*self.clock), self.clock.now_local().date()))
    }

    // ------------------------------------------------------------ refresh
    /// Rescan the project folder without restarting. Nothing the translator typed is dropped by a refresh, but if there is something not yet saved (a failed save, or an edit
    /// still open in the UI) the caller must confirm first.
    pub fn refresh(&mut self, force: bool, client_dirty: bool, discard: bool) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let mut retried: Vec<&'static str> = Vec::new();
        for kind in o.store.unsaved.clone() {
            if let Some(k) = Kind::from_name(kind) {
                if o.store.save(k, clock).is_ok() {
                    retried.push(kind);
                }
            }
        }
        if (!o.store.unsaved.is_empty() || client_dirty) && !discard {
            let what = if o.store.unsaved.is_empty() {
                msg(" (a field being edited)", &[])
            } else {
                let files = o.store.unsaved.iter().copied().collect::<Vec<_>>().join(", ");
                msg(" ({files} could not be written)", &[("files", json!(files))])
            };
            return Ok(json!({
                "needs_confirm": true,
                "message": msg(
                    "You have unsaved changes{what}. Refreshing the files does not discard them from memory, but fields being edited will be overwritten. Continue?",
                    &[("what", what.wire())],
                ).wire(),
            }));
        }
        let merged = o.store.sync_events(clock);
        let rep = o.project.refresh(force, clock).map_err(|e| app_err(e.0))?;
        // changes noticed at a refresh that this app did not make itself (VS Code, another program)
        for (rel, old) in &rep.prev {
            let Some(fi) = o.project.files.get(rel) else { continue };
            let changes = texthistory::diff_files(&old.parsed.entries, &fi.parsed.entries);
            let items: Vec<(String, (String, String), String, String)> = changes.into_iter().map(|c| (rel.clone(), c.key, c.old, c.new)).collect();
            o.texthist.record(&items, "external", clock);
        }
        let moved = if rep.renamed.is_empty() { 0 } else { Project::migrate_metadata(&mut o.store, &rep.renamed, clock) };
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let changed = rep.changed();
        let mut out = match rep.to_json() {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        let problems = if changed { self.persist()? } else { Vec::new() };
        out.insert("merged_events".into(), json!(merged));
        out.insert("moved_metadata".into(), json!(moved));
        out.insert("save_problems".into(), json!(problems));
        out.insert("retried".into(), json!(retried));
        out.insert("needs_confirm".into(), json!(false));
        Ok(Value::Object(out))
    }

    // ------------------------------------------------------------ gated (Pro) features
    pub fn features_status(&self) -> Value {
        json!({"ok": true, "tier": if self.ext.present() { "pro" } else { "free" }, "features": features::status_of(&self.ext), "pack": self.ext.versions(), "problems": self.ext.problems})
    }

    /// The decision for a feature; locked (an error) when the Pro pack does not offer it.
    pub fn gate(&self, feature: &str) -> Res<features::Decision> {
        let d = features::decide(feature, &self.ext);
        if !d.available {
            return Err(ApiError::Locked(feature.to_string()));
        }
        Ok(d)
    }

    /// Call a feature of the Pro pack: locked when the pack does not offer it, the pack's message when it refuses.
    pub fn ext_call(&self, feature: &str, method: &str, payload: &Value) -> Res<Value> {
        self.gate(feature)?;
        self.ext.call(feature, method, payload).map_err(|e| ApiError::App(e.0))
    }

    /// After a store change: derived files + a result the UI can show.
    pub fn changed(&mut self, label: &str) -> Res<Map<String, Value>> {
        let problems = self.persist()?;
        let mut out = Map::new();
        out.insert("ok".into(), json!(true));
        out.insert("problems".into(), json!(problems));
        out.insert("label".into(), json!(label));
        Ok(out)
    }

    pub fn undo(&mut self) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let label = o.store.undo(clock)?.ok_or_else(|| app_err(msg("Nothing to undo.", &[])))?;
        o.val_cache.clear();
        let mut out = self.changed("undo")?;
        out.insert("undone".into(), label.wire());
        Ok(Value::Object(out))
    }

    /// Open one of the fixed project links in the default browser.
    pub fn open_link(&self, link_id: &str) -> Res<Value> {
        match LINKS.iter().find(|(k, _)| *k == link_id) {
            None => Err(app_err(msg("Unknown link.", &[]))),
            Some((_, url)) => Ok(json!({"ok": true, "url": url})),
        }
    }

    // ------------------------------------------------------------ editor
    pub fn open_editor(&self, rel: &str, line: i64) -> Res<Value> {
        let o = self.o()?;
        let path = match o.project.path_of(rel) {
            Ok(p) => p,
            Err(m) => return Ok(json!({"ok": false, "message": m.wire()})),
        };
        if !o.project.files.contains_key(rel) {
            return Ok(json!({"ok": false, "message": msg("Unknown file.", &[]).wire()}));
        }
        let cfg = self.config.get("editor").cloned().unwrap_or_else(|| json!({}));
        Ok(crate::editor::open_in_editor_real(&path, line, &cfg, Some(&o.project.root)))
    }

    // ------------------------------------------------------------ settings / prefs
    pub fn settings(&self) -> Value {
        let mut d = self.config.public();
        if let Some(m) = d.as_object_mut() {
            m.insert("ui".into(), json!({}));
            m.insert("game_gui".into(), self.game_gui());
        }
        d
    }

    /// Validate and store a settings patch. Only known keys are accepted.
    pub fn update_settings(&mut self, patch: &Map<String, Value>) -> Res<Value> {
        let mut clean = Map::new();
        if let Some(ed) = patch.get("editor").and_then(Value::as_object) {
            let pref = ed.get("preferred").map_or("auto".to_string(), py_str);
            if !["auto", "vscode", "notepadpp", "custom", "system"].contains(&pref.as_str()) {
                return Err(app_err(msg("Invalid editor choice.", &[])));
            }
            let custom: String = ed.get("custom").map_or(String::new(), py_str).chars().take(500).collect();
            clean.insert("editor".into(), json!({"preferred": pref, "custom": custom}));
        }
        if let Some(va) = patch.get("validation").and_then(Value::as_object) {
            let mut out = Map::new();
            for k in ["enabled", "show_info", "spelling"] {
                if let Some(v) = va.get(k) {
                    out.insert(k.into(), json!(crate::util::truthy(v)));
                }
            }
            if let Some(tbx) = va.get("textbox").and_then(Value::as_object) {
                let mut to = Map::new();
                if let Some(v) = tbx.get("enabled") {
                    to.insert("enabled".into(), json!(crate::util::truthy(v)));
                }
                for (k, lo, hi) in [("chars_per_line", 20, 400), ("max_lines", 1, 30)] {
                    if let Some(v) = tbx.get(k) {
                        let n = whole(v).ok_or_else(|| invalid(k))?;
                        if !(lo..=hi).contains(&n) {
                            return Err(between(k, json!(lo), json!(hi)));
                        }
                        to.insert(k.into(), json!(n));
                    }
                }
                out.insert("textbox".into(), Value::Object(to));
            }
            if let Some(ln) = va.get("length").and_then(Value::as_object) {
                let mut lo = Map::new();
                let limits: [(&str, f64, f64); 5] = [
                    ("min_source_chars", 0.0, 500.0),
                    ("max_ratio", 1.0, 20.0),
                    ("min_ratio", 0.0, 1.0),
                    ("min_extra_chars", 0.0, 1000.0),
                    ("min_missing_chars", 0.0, 1000.0),
                ];
                for (k, lo_, hi) in limits {
                    if let Some(v) = ln.get(k) {
                        let val = to_float(v).ok_or_else(|| invalid(k))?;
                        if !(lo_ <= val && val <= hi) {
                            let shown =
                                |x: f64| if matches!(k, "min_source_chars" | "min_extra_chars" | "min_missing_chars") { json!(x as i64) } else { json!(x) };
                            let (a, b) = if k == "min_source_chars" || k == "min_extra_chars" || k == "min_missing_chars" {
                                (shown(lo_), shown(hi))
                            } else {
                                (json!(lo_), json!(hi))
                            };
                            return Err(between(k, a, b));
                        }
                        let stored = if matches!(k, "min_source_chars" | "min_extra_chars" | "min_missing_chars") { json!(val as i64) } else { json!(val) };
                        lo.insert(k.into(), stored);
                    }
                }
                if let Some(v) = ln.get("enabled") {
                    lo.insert("enabled".into(), json!(crate::util::truthy(v)));
                }
                out.insert("length".into(), Value::Object(lo));
            }
            clean.insert("validation".into(), Value::Object(out));
        }
        if let Some(sn) = patch.get("snapshots").and_then(Value::as_object) {
            let mut o = Map::new();
            if let Some(v) = sn.get("enabled") {
                o.insert("enabled".into(), json!(crate::util::truthy(v)));
            }
            for (k, lo, hi) in [("interval_hours", 1, 168), ("keep", 3, 100)] {
                if let Some(v) = sn.get(k) {
                    let n = whole(v).ok_or_else(|| invalid(k))?;
                    if !(lo..=hi).contains(&n) {
                        return Err(between(k, json!(lo), json!(hi)));
                    }
                    o.insert(k.into(), json!(n));
                }
            }
            clean.insert("snapshots".into(), Value::Object(o));
        }
        if let Some(m) = patch.get("mt").and_then(Value::as_object) {
            let mut merged = self.config.get("mt").and_then(Value::as_object).cloned().unwrap_or_default();
            let mut o = Map::new();
            if let Some(p) = m.get("provider") {
                let known = p.as_str().is_some_and(|s| s.is_empty() || mt::PROVIDERS.contains(&s));
                if !known {
                    return Err(app_err(msg("Unknown machine translation provider.", &[])));
                }
                o.insert("provider".into(), p.clone());
            }
            for (k, n) in [("base_url", 300), ("model", 100), ("source_lang", 10), ("target_lang", 10)] {
                if let Some(v) = m.get(k) {
                    o.insert(k.into(), json!(crate::util::py_strip(&py_str(v)).chars().take(n).collect::<String>()));
                }
            }
            if let Some(url) = o.get("base_url").and_then(Value::as_str).filter(|u| !u.is_empty()) {
                mt::check_url(url).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
            }
            if let Some(v) = m.get("consent") {
                o.insert("consent".into(), json!(crate::util::truthy(v)));
            }
            if m.get("clear_key").is_some_and(crate::util::truthy) {
                o.insert("api_key".into(), json!(""));
            } else if let Some(key) = m.get("api_key").map(py_str).filter(|k| !crate::util::py_strip(k).is_empty()) {
                o.insert("api_key".into(), json!(crate::util::py_strip(&key).chars().take(300).collect::<String>()));
                // an empty field keeps the stored key
            }
            merged.extend(o);
            clean.insert("mt".into(), Value::Object(merged));
        }
        if let Some(sdk) = patch.get("lint").and_then(Value::as_object).and_then(|li| li.get("sdk")) {
            let text = py_str(sdk);
            let text = crate::util::py_strip(&text).trim_matches('"');
            clean.insert("lint".into(), json!({"sdk": text.chars().take(500).collect::<String>()}));
        }
        if let Some(gl) = patch.get("goals").and_then(Value::as_object) {
            let mut g = Map::new();
            for k in ["translate_per_week", "proofread_per_week"] {
                if let Some(v) = gl.get(k) {
                    let v = if crate::util::truthy(v) { v.clone() } else { json!(0) };
                    let n = whole(&v).ok_or_else(|| app_err(msg("Invalid goal: {k}", &[("k", json!(k))])))?;
                    if !(0..=100000).contains(&n) {
                        return Err(app_err(msg("The goal must be between 0 and 100 000 lines per week (0 = automatic).", &[])));
                    }
                    g.insert(k.into(), json!(n));
                }
            }
            clean.insert("goals".into(), Value::Object(g));
        }
        if let Some(web) = patch.get("web").and_then(Value::as_object) {
            let mut w = Map::new();
            for k in ["repo", "path", "title"] {
                if let Some(v) = web.get(k) {
                    w.insert(k.into(), json!(crate::util::py_strip(&py_str(v)).chars().take(300).collect::<String>()));
                }
            }
            if let Some(p) = w.get("path").and_then(Value::as_str).map(String::from) {
                let cleaned = export::clean_target(&p).map_err(app_err)?;
                w.insert("path".into(), json!(cleaned));
            }
            clean.insert("web".into(), Value::Object(w));
        }
        if let Some(sf) = patch.get("skip_files") {
            let items: Vec<String> = match sf {
                Value::Array(a) => a.iter().map(|x| crate::util::py_strip(&py_str(x)).to_string()).filter(|s| !s.is_empty()).take(200).collect(),
                _ => Vec::new(),
            };
            clean.insert("skip_files".into(), json!(items));
        }
        let clean = Value::Object(clean);
        self.config.update(&clean, &*self.clock).map_err(|e| app_err(msg("The settings cannot be saved: {reason}", &[("reason", e.reason.wire())])))?;
        if let Some(o) = self.open.as_mut() {
            o.val_cache.clear();
            if let Some(list) = clean.get("skip_files").and_then(Value::as_array) {
                let names: Vec<String> = list.iter().map(py_str).collect();
                o.project.set_skip(&names);
            }
        }
        Ok(self.settings())
    }

    pub fn ui_prefs(&self) -> Value {
        self.config.get("ui").cloned().unwrap_or_else(|| json!({}))
    }

    pub fn set_ui_prefs(&mut self, prefs: &Value) -> Res<Value> {
        if !prefs.is_object() || json_dumps(prefs, true).len() > UI_PREF_MAX {
            return Err(app_err(msg("Invalid preferences.", &[])));
        }
        self.config.set("ui", prefs.clone());
        let _ = self.config.save(&*self.clock);
        Ok(json!({"ok": true}))
    }

    /// The findings cache of one file (see `app_validation.rs`).
    pub fn cached(&self, rel: &str) -> Option<&CachedIssues> {
        self.open.as_ref()?.val_cache.get(rel)
    }
}

fn invalid(k: &str) -> ApiError {
    app_err(msg("Invalid value: {k}", &[("k", json!(k))]))
}

fn between(k: &str, lo: Value, hi: Value) -> ApiError {
    app_err(msg("{k} must be between {lo} and {hi}.", &[("k", json!(k)), ("lo", lo), ("hi", hi)]))
}

/// `float(v)` for what a client can send.
fn to_float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(*b as i64 as f64),
        Value::String(s) => crate::pyfmt::float(s),
        _ => None,
    }
    .filter(|f| f.is_finite())
}

/// `int(float(v))`.
fn whole(v: &Value) -> Option<i64> {
    to_float(v).map(|f| f.trunc() as i64)
}

#[allow(dead_code)]
fn message_of(m: Message) -> Text {
    Text::new_msg(m)
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_state(app: &mut App, _a: &Args) -> Res<Answer> {
    Ok(Answer::Json(json!({
        "ok": true, "ready": app.ready(), "notices": app.notices.iter().map(Text::wire).collect::<Vec<_>>(),
        "config_error": app.config.load_error.as_ref().map_or(json!(""), Message::wire), "prefs": app.ui_prefs(), "version": crate::VERSION,
        "project_dir": app.config.get("project_dir").cloned().unwrap_or_else(|| json!("")),
    })))
}

pub fn op_ui_langs(app: &mut App, _a: &Args) -> Res<Answer> {
    Ok(Answer::Json(json!({"ok": true, "langs": app.packs.ui_languages()})))
}

pub fn op_ui_lang(app: &mut App, a: &Args) -> Res<Answer> {
    let mut res = app.packs.ui_language(a.s("code")).map_err(|e| ApiError::App(Text::Plain(e.0)))?;
    let code = res["code"].as_str().unwrap_or("").to_string();
    let mut strings = res["strings"].as_object().cloned().unwrap_or_default();
    strings.extend(app.ext.ui_strings(&code)); // plus the texts of the Pro pack, if there is one
    res["strings"] = Value::Object(strings);
    ok(res)
}

pub fn op_settings(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.settings())
}

pub fn op_project_set(app: &mut App, a: &Args) -> Res<Answer> {
    let path = crate::util::py_strip(a.s("path")).trim_matches('"').to_string();
    app.open_project(&path, a.f("force"))?;
    app.maybe_auto_snapshot()?;
    Ok(Answer::Json(json!({"ok": true})))
}

pub fn op_refresh(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.refresh(a.f("force"), a.f("dirty"), a.f("discard"))?)
}

pub fn op_features(app: &mut App, _a: &Args) -> Res<Answer> {
    plain(app.features_status())
}

pub fn op_undo(app: &mut App, _a: &Args) -> Res<Answer> {
    plain(app.undo()?)
}

pub fn op_open_editor(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.open_editor(a.s("file"), a.i("line"))?)
}

pub fn op_open_url(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.open_link(a.s("id"))?)
}

pub fn op_settings_update(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.update_settings(&a.raw().clone())?)
}

pub fn op_prefs(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.set_ui_prefs(&Value::Object(a.raw().clone()))?)
}
