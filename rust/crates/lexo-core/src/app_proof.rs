//! Port of `reference/python/core/lexo/app_proof.py`: proofreading marks on lines.
use crate::api::{plain, Answer, Args};
use crate::app::{unknown_file, App};
use crate::app_base::{app_err, Res};
use crate::messages::{msg, py_str};
use crate::store::{text_hash, Kind, Label, Mark};
use crate::util::norm_ws;
use serde_json::{json, Value};
use std::path::Path;

impl App {
    /// Mark lines proofread (`ok`), flagged for fixing (`fix`, optional note) or clear the mark (`""`). Only translated lines can be proofread. One undo step.
    /// Returns `{changed, skipped}`.
    pub fn proof_set(&mut self, items: &[Value], state: &str, note: &Value) -> Res<Value> {
        self.need_project()?;
        if !["ok", "fix", ""].contains(&state) {
            return Err(app_err(msg("Invalid proofreading state.", &[])));
        }
        // (file, key, translation of the line)
        let mut plan: Vec<(String, (String, String), String)> = Vec::new();
        let mut skipped: Vec<Value> = Vec::new();
        {
            let o = self.o()?;
            for it in items {
                let rel_v = it.get("file").cloned().unwrap_or(Value::Null);
                let Some(rel) = rel_v.as_str().filter(|r| o.project.files.contains_key(*r)) else {
                    skipped.push(json!([rel_v, msg("unknown file", &[]).wire()]));
                    continue;
                };
                let k = it.get("key").filter(|v| crate::util::truthy(v)).and_then(Value::as_array).cloned().unwrap_or_else(|| vec![json!(""), json!("")]);
                let key = (py_str(&k[0]), norm_ws(&py_str(k.get(1).unwrap_or(&json!("")))));
                match o.project.entries(rel).iter().find(|x| x.key() == key) {
                    None => skipped.push(json!([rel, msg("the line changed in the file – refresh the files", &[]).wire()])),
                    Some(e) if !state.is_empty() && !o.project.is_translated(&o.store, rel, e) => {
                        skipped.push(json!([rel, msg("the line is not translated yet", &[]).wire()]))
                    }
                    Some(e) => plan.push((rel.to_string(), key, e.target.clone())),
                }
            }
        }
        let today = self.clock.now_local().date().format("%Y-%m-%d").to_string();
        let mut changed = 0usize;
        let word = match state {
            "ok" => msg("Proofreading: checked", &[]),
            "fix" => msg("Proofreading: to fix", &[]),
            _ => msg("Proofreading: cleared", &[]),
        };
        let label = Label::Plain(format!("{} ({})", word.text(), plan.len()));
        {
            let clock = &*self.clock;
            let o = self.open.as_mut().unwrap();
            o.store.change(
                label,
                &[Kind::Proof],
                |st| {
                    for (rel, key, target) in &plan {
                        let marks = st.proof.entry(rel.clone()).or_default();
                        let old = marks.get(key).cloned();
                        if state.is_empty() {
                            if marks.shift_remove(key).is_some() {
                                changed += 1;
                            }
                        } else {
                            let keep = old.as_ref().and_then(|m| m.get("n").cloned()).unwrap_or(json!(""));
                            let new: Mark = [
                                ("s".to_string(), json!(state)),
                                ("h".to_string(), json!(text_hash(target))),
                                ("d".to_string(), json!(today)),
                                ("n".to_string(), if note.is_null() { keep } else { note.clone() }),
                            ]
                            .into_iter()
                            .collect();
                            if old.as_ref() != Some(&new) {
                                marks.insert(key.clone(), new);
                                changed += 1;
                            }
                        }
                        if marks.is_empty() {
                            st.proof.shift_remove(rel);
                        }
                    }
                },
                clock,
            )?;
        }
        let mut out = self.changed("proof")?;
        out.insert("changed".into(), json!(changed));
        out.insert("skipped".into(), Value::Array(skipped));
        Ok(Value::Object(out))
    }

    /// Every line of a file up to `line` (inclusive): state `ok` marks translated lines that are not yet proofread (or whose proofreading went stale) as proofread; state
    /// `""` takes the "proofread" mark away again (also stale ones). Lines flagged "to fix" are left alone.
    pub fn proof_upto(&mut self, rel: &str, line: i64, state: &str) -> Res<Value> {
        self.need_project()?;
        if !["ok", ""].contains(&state) {
            return Err(app_err(msg("Invalid proofreading state.", &[])));
        }
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let wanted: [&str; 2] = if state == "ok" { ["", "stale"] } else { ["ok", "stale"] };
        let o = self.o()?;
        let items: Vec<Value> = o
            .project
            .entries(rel)
            .iter()
            .filter(|e| e.line as i64 <= line && o.project.is_translated(&o.store, rel, e) && wanted.contains(&o.project.proof_state(&o.store, rel, e)))
            .map(|e| {
                let k = e.key();
                json!({"file": rel, "key": [k.0, k.1]})
            })
            .collect();
        if items.is_empty() {
            return Ok(json!({"ok": true, "changed": 0, "skipped": [], "problems": []}));
        }
        self.proof_set(&items, state, &Value::Null)
    }

    /// Remove all per-line proofread marks of a file (one undo step).
    pub fn proof_clear_file(&mut self, rel: &str) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let n = o.store.proof.get(rel).map_or(0, |m| m.len());
        let name = Path::new(rel).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        o.store.change(
            Label::Msg(msg("Proofreading: marks removed in {name}", &[("name", json!(name))])),
            &[Kind::Proof],
            |st| {
                st.proof.shift_remove(rel);
            },
            clock,
        )?;
        let mut out = self.changed("proof")?;
        out.insert("changed".into(), json!(n));
        Ok(Value::Object(out))
    }

    /// First translated line (after `after`, in file order) that still needs proofreading.
    pub fn proof_next(&self, file: &str, after: i64) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let files: Vec<String> = if file.is_empty() {
            let mut v: Vec<String> = o.project.files.keys().cloned().collect();
            v.sort_by_key(|r| r.to_lowercase());
            v
        } else {
            vec![file.to_string()]
        };
        for rel in files {
            if !o.project.files.contains_key(&rel) {
                return Err(unknown_file());
            }
            for e in o.project.entries(&rel) {
                if (file.is_empty() || e.line as i64 > after)
                    && o.project.is_translated(&o.store, &rel, e)
                    && ["", "stale"].contains(&o.project.proof_state(&o.store, &rel, e))
                {
                    return Ok(json!({"found": true, "file": rel, "line": e.line}));
                }
            }
        }
        Ok(json!({"found": false}))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_proof_set(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.proof_set(a.l("items"), a.s("state"), a.a("note"))?)
}

pub fn op_proof_upto(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.proof_upto(a.s("file"), a.i("line"), a.s("state"))?)
}

pub fn op_proof_clear_file(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.proof_clear_file(a.s("file"))?)
}

pub fn op_proof_next(app: &mut App, a: &Args) -> Res<Answer> {
    crate::api::ok(app.proof_next(a.s("file"), a.i("after"))?)
}
