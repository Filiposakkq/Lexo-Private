//! Port of `core/lexo/app_memory.py`: translation memory and consistency of repeated originals.
use crate::api::{ok, plain, Answer, Args};
use crate::app::{line_gone, App};
use crate::app_base::{app_err, ApiError, Res};
use crate::fixes;
use crate::memory::{self, Memory};
use crate::messages::{msg, py_str, Text};
use crate::util::sha1_hex;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::rc::Rc;

fn sorted_tags(text: &str) -> Vec<(String, usize)> {
    let mut v = memory::tags_of(text);
    v.sort();
    v
}

impl App {
    /// Index of everything already translated (looked up by the English original). Rebuilt when a file changes.
    pub fn memory(&mut self) -> Res<Rc<Memory>> {
        let o = self.o_mut()?;
        let mut files: Vec<(&String, &str)> = o.project.files.iter().map(|(rel, fi)| (rel, fi.parsed.digest.as_str())).collect();
        files.sort();
        // like Python, the index is rebuilt when a file changes (not when only an approval does)
        let key = sha1_hex(format!("{files:?}").as_bytes());
        if let Some((k, m)) = &o.mem_cache {
            if *k == key {
                return Ok(m.clone());
            }
        }
        let mut rows: Vec<(String, i64, String, String, String)> = Vec::new();
        for (rel, fi) in &o.project.files {
            for e in &fi.parsed.entries {
                if let Some(src) = &e.source {
                    if o.project.is_translated(&o.store, rel, e) {
                        rows.push((rel.clone(), e.line as i64, e.who.clone(), src.clone(), e.target.clone()));
                    }
                }
            }
        }
        let mem = Rc::new(Memory::new(&rows));
        o.mem_cache = Some((key, mem.clone()));
        Ok(mem)
    }

    /// Suggestions for one line from the project's own translations: the same original translated before, and originals that differ by a few words.
    pub fn entry_memory(&mut self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let (source, _) = {
            let o = self.o()?;
            let ent = if o.project.files.contains_key(rel) { o.project.entries(rel).iter().find(|e| e.line as i64 == line) } else { None };
            match ent.and_then(|e| e.source.clone()) {
                Some(s) => (s, ()),
                None => return Err(line_gone()),
            }
        };
        let mem = self.memory()?;
        let mine = sorted_tags(&source);
        let mark = |mut item: Value, src: &str| {
            item["vars_differ"] = json!(sorted_tags(src) != mine);
            item
        };
        let skip = Some((rel, line));
        let exact: Vec<Value> = mem
            .exact_matches(&source, skip)
            .into_iter()
            .map(|g| {
                let (f, l) = (g["lines"][0]["file"].as_str().unwrap_or("").to_string(), g["lines"][0]["line"].as_i64().unwrap_or(0));
                let row = mem.rows.iter().find(|r| r.file == f && r.line == l);
                let src = row.map_or(source.clone(), |r| r.source.clone());
                mark(g, &src)
            })
            .collect();
        let similar: Vec<Value> = mem
            .similar(&source, skip, 5, 0.7)
            .into_iter()
            .map(|x| {
                let src = x["source"].as_str().unwrap_or("").to_string();
                mark(x, &src)
            })
            .collect();
        Ok(json!({"exact": exact, "similar": similar}))
    }

    /// Originals translated in several different ways (exactly the same original text; see `Memory::groups`).
    pub fn consistency(&mut self, loose: bool, by_speaker: bool) -> Res<Value> {
        self.need_project()?;
        let groups = self.memory()?.groups(500, loose, by_speaker);
        let lines: i64 = groups.iter().map(|g| g["total"].as_i64().unwrap_or(0)).sum();
        Ok(json!({"total": groups.len(), "lines": lines, "groups": groups}))
    }

    /// Give the chosen lines of one original the same chosen translation (one undoable step). `lines` = `[{file, line}]` picks exactly which lines change; without it every
    /// line of the group does.
    pub fn consistency_unify(&mut self, key: &str, target: &str, lines: &Value, loose: bool, by_speaker: bool) -> Res<Value> {
        self.need_project()?;
        let mem = self.memory()?;
        let groups = mem.groups(1_000_000, loose, by_speaker);
        let Some(group) = groups.iter().find(|g| g["key"].as_str() == Some(key)) else {
            return Err(app_err(msg("The group has changed in the meantime – refresh the files and try again.", &[])));
        };
        let chosen: Option<BTreeSet<(String, i64)>> = lines
            .as_array()
            .filter(|a| !a.is_empty())
            .map(|a| a.iter().map(|x| (py_str(&x["file"]), x["line"].as_i64().or_else(|| py_str(&x["line"]).parse().ok()).unwrap_or(0))).collect());
        let mut edits: Vec<(String, usize, String)> = Vec::new();
        for v in group["variants"].as_array().into_iter().flatten() {
            for l in v["lines"].as_array().into_iter().flatten() {
                let (f, n) = (l["file"].as_str().unwrap_or("").to_string(), l["line"].as_i64().unwrap_or(0));
                if chosen.as_ref().is_none_or(|c| c.contains(&(f.clone(), n))) {
                    edits.push((f, n.max(0) as usize, target.to_string()));
                }
            }
        }
        if edits.is_empty() {
            return Err(app_err(msg("Nothing to unify.", &[])));
        }
        let files = edits.iter().map(|e| e.0.clone()).collect::<BTreeSet<_>>().len();
        self.snapshot_for_bulk(edits.len(), files);
        let source: String = group["source"].as_str().unwrap_or("").chars().take(40).collect();
        let label = msg("Unifying the translation: {text}", &[("text", json!(source))]);
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let res = fixes::apply_edits(&o.project, &o.pack, &o.local_dir, &edits, &label, clock);
        let _ = o.project.refresh(true, clock);
        o.val_cache.clear();
        let res = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let details: Vec<Value> = res.to_json()["details"].as_array().cloned().unwrap_or_default();
        self.log_text(&details, "unify");
        let mut out = self.changed("unify")?;
        out.insert("changed".into(), json!(res.changed));
        out.insert("files".into(), json!(res.files));
        out.insert("backups".into(), json!(res.backups));
        Ok(Value::Object(out))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_entry_memory(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_memory(a.s("file"), a.i("line"))?)
}

pub fn op_consistency(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.consistency(a.f("loose"), a.f("by_speaker"))?)
}

pub fn op_consistency_unify(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.consistency_unify(a.s("key"), a.s("target"), a.a("lines"), a.f("loose"), a.f("by_speaker"))?)
}
