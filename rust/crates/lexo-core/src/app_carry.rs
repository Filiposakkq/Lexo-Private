//! Two Pro features that look up translations by the ORIGINAL text and write what the user ticks, as one undoable bulk change (like "replace"):
//!
//! * `carry.over` – carry a translation over to a new version of a game: the open project is the NEW version (its lines are untranslated), the folder the user names is the OLD,
//!   translated one;
//! * `memory.fuzzy` – fill untranslated lines from the project's OWN translations of similar originals (the translation memory, not only for one line but for all of them).
//!
//! Core reads the lines and hands plain data to the Pro pack, which finds the translation of every untranslated line; Core shows what was found and writes the lines the user ticks.
//! Lines whose original is not the same ("near") can be marked "to fix" so that they are checked. These calls are not in the contract of Core: they belong to the application around it.
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::fixes;
use crate::messages::{msg, py_str, Text};
use crate::project::{check_project_dir, find_translation_folder, Project};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const FEATURE: &str = "carry.over";
pub const FEATURE_FILL: &str = "memory.fuzzy";

impl App {
    /// The matches of the untranslated lines of the open project: in the old project at `folder` (`carry.over`), or in the project itself (`memory.fuzzy`, `folder` is `None`).
    /// `{items, counts, limited?}`; every item carries the place of the line.
    fn match_plan(&mut self, folder: Option<&str>, min_ratio: f64) -> Res<Value> {
        self.need_project()?;
        let feature = if folder.is_some() { FEATURE } else { FEATURE_FILL };
        if !self.ext.decide(feature).available {
            let title = if folder.is_some() { "Carry a translation over to a new version" } else { "Fill from similar translations" };
            return Err(app_err(msg("“{title}” is a Pro feature.", &[("title", json!(title))])));
        }
        // the translations to look in
        let mut old_lines: Vec<Value> = Vec::new();
        if let Some(old_path) = folder {
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
        } else {
            let o = self.o()?;
            for rel in o.project.files.keys() {
                for e in o.project.entries(rel) {
                    if (e.unit != "dialogue" && e.unit != "string") || e.target.trim().is_empty() || !o.project.is_translated(&o.store, rel, e) {
                        continue;
                    }
                    if let Some(src) = &e.source {
                        old_lines.push(json!({"id": format!("{rel}\t{}", e.line), "who": e.src_who, "source": src, "target": e.target}));
                    }
                }
            }
            if old_lines.is_empty() {
                return Err(app_err(msg("The project has no translated lines yet.", &[])));
            }
        }
        // the lines to translate
        let o = self.o()?;
        let mut new_lines: Vec<Value> = Vec::new();
        let mut places: BTreeMap<String, (String, i64, String, String)> = BTreeMap::new();
        for rel in o.project.files.keys() {
            for e in o.project.entries(rel) {
                // still to translate: an empty translation, or a line that is pending and was not approved as it is
                let todo = e.target.trim().is_empty() || (e.pending() && !o.project.is_translated(&o.store, rel, e));
                if !todo || (e.unit != "dialogue" && e.unit != "string") {
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
        let mut answer = self.ext_call(feature, "match", &json!({"old": old_lines, "new": new_lines, "min_ratio": min_ratio}))?;
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
        let mut plan = self.match_plan(Some(old_path), min_ratio)?;
        plan["ok"] = json!(true);
        Ok(plan)
    }

    /// What would be filled from the project's own translations, nothing is written.
    pub fn fill_preview(&mut self, min_ratio: f64) -> Res<Value> {
        let mut plan = self.match_plan(None, min_ratio)?;
        plan["ok"] = json!(true);
        Ok(plan)
    }

    /// Writes the translations of the ticked lines (`selections`: `[{file, line}]`) as one undoable step with backups.
    /// With `mark`, the lines whose original had changed ("near") are marked "to fix" with a note, so that they show in the filter of lines to check; lines with the same original are not.
    pub fn carry_apply(&mut self, old_path: &str, min_ratio: f64, selections: &[Value], mark: bool) -> Res<Value> {
        self.apply_plan(Some(old_path), min_ratio, selections, mark)
    }

    /// The same for the lines filled from the project's own translations.
    pub fn fill_apply(&mut self, min_ratio: f64, selections: &[Value], mark: bool) -> Res<Value> {
        self.apply_plan(None, min_ratio, selections, mark)
    }

    fn apply_plan(&mut self, folder: Option<&str>, min_ratio: f64, selections: &[Value], mark: bool) -> Res<Value> {
        let chosen: BTreeSet<(String, i64)> = selections.iter().map(|x| (py_str(&x["file"]), x["line"].as_i64().unwrap_or(0))).collect();
        if chosen.is_empty() {
            return Err(app_err(msg("No line is selected.", &[])));
        }
        let plan = self.match_plan(folder, min_ratio)?;
        let picked = |i: &&Value| chosen.contains(&(py_str(&i["file"]), i["line"].as_i64().unwrap_or(0)));
        let plan_near: Vec<(String, usize)> = plan["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["kind"] == "near")
            .filter(picked)
            .map(|i| (py_str(&i["file"]), i["line"].as_u64().unwrap_or(0) as usize))
            .collect();
        let edits: Vec<(String, usize, String)> = plan["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(picked)
            .map(|i| (py_str(&i["file"]), i["line"].as_u64().unwrap_or(0) as usize, py_str(&i["target"])))
            .collect();
        if edits.is_empty() {
            return Err(app_err(msg("The selected lines changed in the meantime – show the preview again.", &[])));
        }
        let (label, via, note) = if folder.is_some() {
            (
                msg("Translation carried over from the previous version", &[]),
                "carry",
                msg("Carried over from the previous version: the original had changed", &[]),
            )
        } else {
            (msg("Filled from similar translations", &[]), "fill", msg("Filled from a similar original: check it", &[]))
        };
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
        self.log_text(&details, via);
        let mut out = self.changed(via)?;
        out.insert("changed".into(), json!(res.changed));
        out.insert("files".into(), json!(res.files));
        // the lines that came from a changed original are marked for a check (an own undo step of the marks)
        let mut marked = 0;
        if mark {
            let items: Vec<Value> = {
                let o = self.o()?;
                plan_near
                    .iter()
                    .filter_map(|(rel, line)| {
                        let e = o.project.entries(rel).iter().find(|e| e.line == *line)?;
                        let key = e.key();
                        Some(json!({"file": rel, "key": [key.0, key.1]}))
                    })
                    .collect()
            };
            if !items.is_empty() {
                marked = self.proof_set(&items, "fix", &json!(note.text()))?["changed"].as_u64().unwrap_or(0);
            }
        }
        out.insert("marked".into(), json!(marked));
        Ok(Value::Object(out))
    }
}
