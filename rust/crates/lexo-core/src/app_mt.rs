//! Port of `core/lexo/app_mt.py`: machine translation of lines.
//!
//! The network is a function of the application (`App::mt_http`): the programs (`lexo-server`, `lexo-app`) set `net::post`, the tests and the golden runs leave it out (or put a fake) and
//! then the operations that send text give the error "no connection".
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::fixes;
use crate::messages::{msg, py_str, Text};
use crate::mt;
use crate::rpy::Entry;
use crate::validator::{self, Checks};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// Lines per request: the UI loops, so it can show progress and stop.
const MT_BATCH: usize = 10;

fn file_line(x: &Value) -> (String, i64) {
    (py_str(&x["file"]), x["line"].as_i64().or_else(|| py_str(&x["line"]).trim().parse().ok()).unwrap_or(0))
}

impl App {
    /// The machine translation settings; an empty target language means "the language of the project" (its pack, or the name of its tl folder).
    pub fn mt_config(&self) -> Map<String, Value> {
        let mut cfg = self.config.get("mt").and_then(Value::as_object).cloned().unwrap_or_default();
        if !cfg.get("target_lang").is_some_and(crate::util::truthy) {
            let (language, folder) = match self.open.as_ref() {
                Some(o) => (o.store.layout.language.clone(), o.project.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                None => (String::new(), String::new()),
            };
            cfg.insert("target_lang".into(), json!(mt::target_language(&language, &folder)));
        }
        cfg
    }

    /// What the validator says about `text` as the translation of `ent` (information is left out).
    fn mt_issues(&self, ent: &Entry, text: &str) -> Res<Vec<Value>> {
        let mut probe = ent.clone();
        probe.target = text.to_string();
        probe.category = "";
        let (length, textbox, terms) = (self.length_cfg(), self.textbox_cfg(), self.terms());
        let o = self.o()?;
        let checks = Checks { length_cfg: Some(&length), terms: &terms, spell: None, textbox: Some(&textbox) };
        Ok(validator::check_entry(&probe, &o.pack, &checks).iter().filter(|i| i.severity != "info").map(|i| i.to_json()).collect())
    }

    /// Run the translation with the function of the application for the network.
    fn mt_translate(&self, cfg: &Map<String, Value>, source: &str, glossary: &[(String, String)], context: &[String]) -> Result<mt::Translation, mt::MtError> {
        match &self.mt_http {
            Some(http) => mt::translate(cfg, source, glossary, context, &**http),
            None => {
                let no_net = |_: &str, _: &[(String, String)], _: &[u8]| -> Result<(u16, String), mt::MtError> {
                    Err(mt::MtError(msg("The connection to the service failed: {reason}", &[("reason", json!("this build has no network client yet"))])))
                };
                mt::translate(cfg, source, glossary, context, &no_net)
            }
        }
    }

    fn mt_one(&self, ents: &[Entry], idx: usize, cfg: &Map<String, Value>, terms: &[(String, String)]) -> Res<Value> {
        let ent = &ents[idx];
        let source = ent.source.as_deref().unwrap_or("");
        let context: Vec<String> = ents[idx.saturating_sub(2)..idx].iter().filter_map(|e| e.source.clone()).filter(|s| !s.is_empty()).collect();
        let local: Vec<(String, String)> = terms.iter().filter(|(en, _)| validator::term_in_text(source, en)).cloned().collect();
        let res = self.mt_translate(cfg, source, &local, &context).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        Ok(
            json!({"suggestion": res.text, "warnings": res.warnings.iter().map(|w| w.wire()).collect::<Vec<_>>(), "issues": self.mt_issues(ent, &res.text)?, "provider": res.provider}),
        )
    }

    /// A machine translation SUGGESTION for one line (only after consent and a key in the settings; never saved).
    pub fn mt_suggest(&self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let ents: &[Entry] = if o.project.files.contains_key(rel) { o.project.entries(rel) } else { &[] };
        let idx = ents.iter().position(|e| e.line as i64 == line && e.source.is_some()).ok_or_else(crate::app::line_gone)?;
        self.mt_one(ents, idx, &self.mt_config(), &self.terms())
    }

    /// Which lines a batch would translate: the given lines, or every line of a file (all files when empty). Only not yet translated lines by default. The Pro pack cuts the
    /// list to its limit; returns the count and the characters that would be sent.
    pub fn mt_targets(&self, file: &str, lines: &Value, only_todo: bool) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let want: Option<BTreeSet<(String, i64)>> = lines.as_array().filter(|a| !a.is_empty()).map(|a| a.iter().map(file_line).collect());
        let files: Vec<String> = match &want {
            Some(w) => w.iter().map(|x| x.0.clone()).collect::<BTreeSet<_>>().into_iter().collect(),
            None if file.is_empty() => o.project.files.keys().cloned().collect(),
            None => vec![file.to_string()],
        };
        let mut cands: Vec<Value> = Vec::new();
        for rel in files {
            if !o.project.files.contains_key(&rel) {
                return Err(crate::app::unknown_file());
            }
            for e in o.project.entries(&rel) {
                let Some(src) = e.source.as_deref().filter(|s| !crate::util::py_strip(s).is_empty()) else { continue };
                if want.as_ref().is_some_and(|w| !w.contains(&(rel.clone(), e.line as i64))) {
                    continue;
                }
                if only_todo && self.entry_row(&rel, e) != "todo" {
                    continue;
                }
                cands.push(json!({"file": rel, "line": e.line, "chars": src.chars().count()}));
            }
        }
        self.ext_call("mt.batch", "targets", &json!({"items": cands}))
    }

    /// Suggestions for up to `MT_BATCH` lines (the Pro pack translates). Nothing is written. A failed line carries its own error; a rejected key, missing consent or a broken
    /// connection stops the batch (the same message for every line would be useless).
    pub fn mt_batch(&self, items: &Value) -> Res<Value> {
        self.need_project()?;
        self.gate("mt.batch")?;
        let items: Vec<&Value> = items.as_array().into_iter().flatten().take(MT_BATCH).collect();
        let (cfg, terms) = (self.mt_config(), self.terms());
        let o = self.o()?;
        // (file, line, the entries of the file and the index of the line)
        #[allow(clippy::type_complexity)]
        let mut plan: Vec<(String, i64, Option<(&[Entry], usize)>)> = Vec::new();
        let mut reqs: Vec<Value> = Vec::new();
        for it in items {
            let (rel, line) = file_line(it);
            let ents: &[Entry] = if o.project.files.contains_key(&rel) { o.project.entries(&rel) } else { &[] };
            let Some(idx) = ents.iter().position(|e| e.line as i64 == line && e.source.is_some()) else {
                plan.push((rel, line, None));
                continue;
            };
            let ent = &ents[idx];
            let source = ent.source.as_deref().unwrap_or("");
            let context: Vec<&str> = ents[idx.saturating_sub(2)..idx].iter().filter_map(|e| e.source.as_deref()).filter(|s| !s.is_empty()).collect();
            let local: Map<String, Value> =
                terms.iter().filter(|(en, _)| validator::term_in_text(source, en)).map(|(en, cz)| (en.clone(), json!(cz))).collect();
            let mut req = json!({"id": reqs.len(), "source": source, "context": context, "terms": local});
            if let Some(profile) = self.profile_for(&ent.who) {
                req["profile"] = profile; // the profile of the speaker (Pro: the pack puts it into the request of the language model)
            }
            reqs.push(req);
            plan.push((rel, line, Some((ents, idx))));
        }
        let res = if reqs.is_empty() {
            json!({"results": [], "stopped": ""})
        } else {
            self.ext_call("mt.batch", "translate", &json!({"cfg": cfg, "requests": reqs}))?
        };
        let got: Vec<Value> = res["results"].as_array().cloned().unwrap_or_default();
        let stopped = res["stopped"].clone();
        let (mut results, mut k) = (Vec::new(), 0usize);
        for (rel, line, found) in &plan {
            if crate::util::truthy(&stopped) && k >= got.len() {
                break; // the batch stopped before this line
            }
            let Some((ents, idx)) = found else {
                results.push(json!({"file": rel, "line": line, "error": msg("The line is no longer in the file.", &[]).wire()}));
                continue;
            };
            let r = &got[k];
            k += 1;
            if let Some(err) = r.get("error") {
                results.push(json!({"file": rel, "line": line, "error": err}));
                continue;
            }
            let ent = &ents[*idx];
            let key = ent.key();
            let suggestion = py_str(&r["suggestion"]);
            results.push(
                json!({"file": rel, "line": line, "key": [key.0, key.1], "who": ent.who, "source": ent.source, "current": ent.target, "suggestion": suggestion,
                "warnings": r["warnings"], "issues": self.mt_issues(ent, &suggestion)?, "provider": r["provider"]}),
            );
        }
        Ok(json!({"results": results, "stopped": stopped}))
    }

    /// Write the ticked suggestions as one undoable step. Approved lines are never overwritten.
    pub fn mt_apply(&mut self, selections: &Value) -> Res<Value> {
        self.need_project()?;
        let (mut edits, mut skipped): (Vec<(String, usize, String)>, usize) = (Vec::new(), 0);
        {
            let o = self.o()?;
            for x in selections.as_array().into_iter().flatten() {
                let (rel, line) = file_line(x);
                let text = x.get("text").map(py_str).unwrap_or_default();
                let ent = if o.project.files.contains_key(&rel) { o.project.entries(&rel).iter().find(|e| e.line as i64 == line) } else { None };
                match ent {
                    Some(e) if !crate::util::py_strip(&text).is_empty() && self.entry_row(&rel, e) != "approved" => {
                        edits.push((rel, line.max(0) as usize, text))
                    }
                    _ => skipped += 1,
                }
            }
        }
        if edits.is_empty() {
            return Err(app_err(msg("Nothing to write (the lines changed or are approved).", &[])));
        }
        let nfiles = edits.iter().map(|e| e.0.clone()).collect::<BTreeSet<_>>().len();
        self.snapshot_for_bulk(edits.len(), nfiles);
        let label = msg("Machine translation ({n} lines)", &[("n", json!(edits.len()))]);
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let res = fixes::apply_edits(&o.project, &o.pack, &o.local_dir, &edits, &label, clock);
        let _ = o.project.refresh(true, clock);
        o.val_cache.clear();
        let res = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let details: Vec<Value> = res.to_json()["details"].as_array().cloned().unwrap_or_default();
        self.log_text(&details, "mt");
        let mut out = self.changed("mt")?;
        out.insert("changed".into(), json!(res.changed));
        out.insert("files".into(), json!(res.files));
        out.insert("skipped".into(), json!(skipped));
        out.insert("backups".into(), json!(res.backups));
        Ok(Value::Object(out))
    }
}

pub fn op_entry_mt(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.mt_suggest(a.s("file"), a.i("line"))?)
}

pub fn op_mt_targets(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.mt_targets(a.s("file"), a.a("lines"), a.f("only_todo"))?)
}

pub fn op_mt_batch(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.mt_batch(a.a("items"))?)
}

pub fn op_mt_apply(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.mt_apply(a.a("selections"))?)
}
