//! Style check with a language model (the Pro feature `style.check`): the lines that are translated are sent, with their original, the profile of the speaker and the lines before them,
//! to the language model that the user set up for the machine translation; the answer says what is wrong (the form of address, the gender forms, the tone, the meaning) and gives a
//! corrected text. Nothing is written here: the ticked fixes are written with `mt/apply` (one undoable step, approved lines are never overwritten). These calls are not in the contract of
//! Core: they belong to the application around it.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::messages::{msg, py_str};
use crate::rpy::Entry;
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const FEATURE: &str = "style.check";

/// Lines per request: the UI loops, so it can show progress and stop (a request has a model call for each line).
const STYLE_BATCH: usize = 5;

fn file_line(x: &Value) -> (String, i64) {
    (py_str(&x["file"]), x["line"].as_i64().or_else(|| py_str(&x["line"]).trim().parse().ok()).unwrap_or(0))
}

impl App {
    fn style_gate(&self) -> Res<()> {
        if self.ext.decide(FEATURE).available {
            Ok(())
        } else {
            Err(app_err(msg("“{title}” is a Pro feature.", &[("title", json!("Style check"))])))
        }
    }

    /// Which lines a check would look at: the given lines, or every translated line of a file (all files when empty). The Pro pack cuts the list to its limit; returns the count and the
    /// characters that would be sent.
    pub fn style_targets(&self, file: &str, lines: &Value) -> Res<Value> {
        self.need_project()?;
        self.style_gate()?;
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
                if (e.unit != "dialogue" && e.unit != "string") || e.target.trim().is_empty() || !o.project.is_translated(&o.store, &rel, e) {
                    continue;
                }
                if want.as_ref().is_some_and(|w| !w.contains(&(rel.clone(), e.line as i64))) {
                    continue;
                }
                cands.push(json!({"file": rel, "line": e.line, "chars": src.chars().count() + e.target.chars().count()}));
            }
        }
        self.ext_call(FEATURE, "targets", &json!({"items": cands}))
    }

    /// Verdicts for up to `STYLE_BATCH` lines (the Pro pack asks the model). Nothing is written. A failed line carries its own error; a rejected key, missing consent, a service that is
    /// not a language model or a broken connection stops the batch.
    pub fn style_batch(&self, items: &Value) -> Res<Value> {
        self.need_project()?;
        self.style_gate()?;
        let items: Vec<&Value> = items.as_array().into_iter().flatten().take(STYLE_BATCH).collect();
        let cfg = self.mt_config();
        let o = self.o()?;
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
            let context: Vec<&str> = ents[idx.saturating_sub(2)..idx].iter().filter_map(|e| e.source.as_deref()).filter(|s| !s.is_empty()).collect();
            let mut req = json!({"id": reqs.len(), "who": ent.who, "source": ent.source, "target": ent.target, "context": context});
            if let Some(profile) = self.profile_for(&ent.who) {
                req["profile"] = profile; // the pack turns it into sentences (when the feature of the profiles is there)
            }
            reqs.push(req);
            plan.push((rel, line, Some((ents, idx))));
        }
        let res =
            if reqs.is_empty() { json!({"results": [], "stopped": ""}) } else { self.ext_call(FEATURE, "check", &json!({"cfg": cfg, "requests": reqs}))? };
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
            let fix = py_str(&r["fix"]);
            // what the validator says about the fixed text (a fix must not lose a tag or a variable)
            let issues = if fix.is_empty() { Vec::new() } else { self.mt_issues(ent, &fix)? };
            results.push(json!({"file": rel, "line": line, "key": [key.0, key.1], "who": ent.who, "source": ent.source, "current": ent.target,
                "ok": r["ok"], "remarks": r["issues"], "fix": fix, "issues": issues}));
        }
        Ok(json!({"results": results, "stopped": stopped}))
    }
}
