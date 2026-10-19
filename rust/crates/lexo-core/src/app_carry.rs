//! Carry a translation over to a new version of a game (the Pro feature `carry.over`).
//!
//! The open project is the NEW version (its lines are untranslated), the folder the user names is the OLD, translated one. Core reads both and hands plain lines to the Pro pack,
//! which finds the translation of every new line; Core shows what was found and writes the lines the user ticks, as one undoable bulk change (like "replace").
//! These calls are not in the contract of Core: they belong to the application around it.
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::fixes;
use crate::messages::{msg, py_str, Text};
use crate::project::{check_project_dir, find_translation_folder, Project};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const FEATURE: &str = "carry.over";

impl App {
    /// The matches of the lines of the open project in the old project at `old_path`: `{items, counts, limited?}`; every item carries the place of the new line.
    fn carry_plan(&mut self, old_path: &str, min_ratio: f64) -> Res<Value> {
        self.need_project()?;
        if !self.ext.decide(FEATURE).available {
            return Err(app_err(msg("“{title}” is a Pro feature.", &[("title", json!("Carry a translation over to a new version"))])));
        }
        let (ok_, problem) = check_project_dir(old_path);
        if !ok_ {
            return Err(app_err(problem.unwrap_or_else(|| msg("The folder cannot be used.", &[]))));
        }
        let (root, _) = find_translation_folder(Path::new(old_path)).map_err(app_err)?;
        let clock = &*self.clock;
        let o = self.o()?;
        if root.canonicalize().ok() == o.project.root.canonicalize().ok() {
            return Err(app_err(msg("Choose the folder of the OLD version, not the open project.", &[])));
        }
        let mut old = Project::new(&root, &[], None, &o.pack);
        old.refresh(true, clock).map_err(|e| app_err(e.0))?;
        let mut old_lines: Vec<Value> = Vec::new();
        for rel in old.files.keys() {
            for e in old.entries(rel) {
                if e.pending() || e.target.trim().is_empty() {
                    continue;
                }
                if let Some(src) = &e.source {
                    old_lines.push(json!({"id": format!("{rel}\t{}", e.line), "who": e.src_who, "source": src, "target": e.target}));
                }
            }
        }
        if old_lines.is_empty() {
            return Err(app_err(msg("The old version has no translated lines.", &[])));
        }
        let mut new_lines: Vec<Value> = Vec::new();
        let mut places: BTreeMap<String, (String, i64, String, String)> = BTreeMap::new();
        for rel in o.project.files.keys() {
            for e in o.project.entries(rel) {
                if !(e.pending() || e.target.trim().is_empty()) || (e.unit != "dialogue" && e.unit != "string") {
                    continue;
                }
                if let Some(src) = &e.source {
                    let id = format!("{rel}\t{}", e.line);
                    new_lines.push(json!({"id": id, "who": e.src_who, "source": src}));
                    places.insert(id, (rel.clone(), e.line as i64, e.who.clone(), src.clone()));
                }
            }
        }
        let old_source: BTreeMap<String, String> = old_lines.iter().map(|l| (py_str(&l["id"]), py_str(&l["source"]))).collect();
        let mut answer = self.ext_call(FEATURE, "match", &json!({"old": old_lines, "new": new_lines, "min_ratio": min_ratio}))?;
        let items: Vec<Value> = answer["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let (file, line, who, source) = places.get(&py_str(&m["new"]))?.clone();
                let mut item = m.clone();
                item["file"] = json!(file);
                item["line"] = json!(line);
                item["who"] = json!(who);
                item["source"] = json!(source);
                item["old_source"] = json!(old_source.get(&py_str(&m["old"])).cloned().unwrap_or_default());
                Some(item)
            })
            .collect();
        answer["items"] = json!(items);
        Ok(answer)
    }

    /// What would be carried over, nothing is written.
    pub fn carry_preview(&mut self, old_path: &str, min_ratio: f64) -> Res<Value> {
        let mut plan = self.carry_plan(old_path, min_ratio)?;
        plan["ok"] = json!(true);
        Ok(plan)
    }

    /// Writes the translations of the ticked lines (`selections`: `[{file, line}]`) as one undoable step with backups.
    pub fn carry_apply(&mut self, old_path: &str, min_ratio: f64, selections: &[Value]) -> Res<Value> {
        let chosen: BTreeSet<(String, i64)> = selections.iter().map(|x| (py_str(&x["file"]), x["line"].as_i64().unwrap_or(0))).collect();
        if chosen.is_empty() {
            return Err(app_err(msg("No line is selected.", &[])));
        }
        let plan = self.carry_plan(old_path, min_ratio)?;
        let edits: Vec<(String, usize, String)> = plan["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| chosen.contains(&(py_str(&i["file"]), i["line"].as_i64().unwrap_or(0))))
            .map(|i| (py_str(&i["file"]), i["line"].as_u64().unwrap_or(0) as usize, py_str(&i["target"])))
            .collect();
        if edits.is_empty() {
            return Err(app_err(msg("The selected lines changed in the meantime – show the preview again.", &[])));
        }
        let label = msg("Translation carried over from the previous version", &[]);
        let nfiles = edits.iter().map(|e| e.0.clone()).collect::<BTreeSet<_>>().len();
        self.snapshot_for_bulk(edits.len(), nfiles);
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let res = fixes::apply_edits(&o.project, &o.pack, &o.local_dir, &edits, &label, clock);
        let _ = o.project.refresh(true, clock);
        o.val_cache.clear();
        let res = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let details: Vec<Value> = res.to_json()["details"].as_array().cloned().unwrap_or_default();
        self.log_text(&details, "carry");
        let mut out = self.changed("carry")?;
        out.insert("changed".into(), json!(res.changed));
        out.insert("files".into(), json!(res.files));
        Ok(Value::Object(out))
    }
}
