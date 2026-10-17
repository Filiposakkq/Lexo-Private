//! Port of `core/lexo/app_fixes.py`: safe fixes, replace in files, undo of the last file change.
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::fixes::{self, Planned};
use crate::messages::{msg, py_str, Message, Text};
use crate::rpy::Entry;
use crate::textedit;
use crate::texthistory;
use crate::validator::{self, Checks};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const FIX_KINDS: [&str; 3] = ["speaker", "spaces", "typo"];

fn fix_label(kind: &str) -> String {
    match kind {
        "speaker" => msg("speaker / attributes", &[]).text(),
        "spaces" => msg("double spaces", &[]).text(),
        _ => msg("typos", &[]).text(),
    }
}

/// `{(file, line)}` or `{(file, line, kind)}` from what a client sent: `int(x["line"])` and `str(x["file"])`.
fn file_line(x: &Value) -> (String, i64) {
    (py_str(&x["file"]), x["line"].as_i64().or_else(|| py_str(&x["line"]).trim().parse().ok()).unwrap_or(0))
}

impl App {
    /// The lines the Pro pack looks at, as plain data. Core knows from its own checks where a speaker is wrong or a spelling hint exists; the pack decides what the fix is.
    fn fix_candidates(&mut self, kinds: &[String], wanted: Option<&BTreeSet<(String, i64)>>, file: &str) -> Res<Value> {
        let spell = if kinds.iter().any(|k| k == "typo") { self.spell() } else { None };
        let o = self.o()?;
        let (mut speaker, mut text): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
        let mut rels: Vec<&String> = o.project.files.keys().collect();
        rels.sort_by_key(|r| r.to_lowercase());
        for rel in rels {
            if !file.is_empty() && rel != file {
                continue;
            }
            for e in o.project.entries(rel) {
                if wanted.is_some_and(|w| !w.contains(&(rel.clone(), e.line as i64))) {
                    continue;
                }
                let eligible = e.source.is_some() && !(e.pending() && !e.category.is_empty());
                if kinds.iter().any(|k| k == "speaker") && e.unit == "dialogue" && eligible {
                    let issues = validator::check_entry(e, &o.pack, &Checks::default());
                    if issues.iter().any(|i| i.code == "speaker-changed" || i.code == "suffix-changed") {
                        speaker.push(
                            json!({"file": rel, "line": e.line, "src_who": e.src_who, "src_suffix": e.src_suffix, "cur_raw": e.cur_raw, "src_raw": e.src_raw}),
                        );
                    }
                }
                if (e.unit == "dialogue" || e.unit == "string") && eligible && !e.target.is_empty() && kinds.iter().any(|k| k == "spaces" || k == "typo") {
                    let mut issues: Vec<Value> = Vec::new();
                    if let (Some(s), Some(src)) = (&spell, e.source.as_deref()) {
                        use validator::SpellCheck;
                        issues = s
                            .check(src, &e.target)
                            .iter()
                            .map(|i| json!({"word": i.extra.get("word"), "better": i.extra.get("better"), "typo_kind": i.extra.get("typo_kind").cloned().unwrap_or(json!(""))}))
                            .collect();
                    }
                    if (kinds.iter().any(|k| k == "spaces") && e.target.contains("  ")) || !issues.is_empty() {
                        text.push(json!({"file": rel, "line": e.line, "source": e.source, "target": e.target, "issues": issues}));
                    }
                }
            }
        }
        Ok(json!({"kinds": kinds, "speaker": speaker, "text": text}))
    }

    /// Every safe fix that could be applied (the Pro pack plans them). Nothing is written.
    pub fn fix_preview(&mut self, items: &[Value], file: &str, kinds: &[Value]) -> Res<Value> {
        self.need_project()?;
        let mut kinds: Vec<String> = kinds.iter().map(py_str).filter(|k| FIX_KINDS.contains(&k.as_str())).collect();
        if kinds.is_empty() {
            kinds = vec!["speaker".to_string()];
        }
        let wanted: Option<BTreeSet<(String, i64)>> = (!items.is_empty()).then(|| items.iter().map(file_line).collect());
        let payload = self.fix_candidates(&kinds, wanted.as_ref(), file)?;
        self.ext_call("fixes.safe", "plan", &payload)
    }

    /// Apply fixes. `selections` = `[{file, line, kind}]` picks exactly what the user ticked in the preview; without it every applicable fix of `kinds` (default: speaker) is
    /// applied (older callers).
    pub fn fix_apply(&mut self, items: &[Value], file: &str, kinds: &[Value], selections: Option<&Vec<Value>>) -> Res<Value> {
        self.need_project()?;
        #[allow(clippy::type_complexity)]
        let (items, kinds, chosen): (Vec<Value>, Vec<Value>, Option<BTreeSet<(String, i64, String)>>) = match selections {
            Some(sel) => {
                let chosen: BTreeSet<(String, i64, String)> = sel.iter().map(|x| (file_line(x).0, file_line(x).1, py_str(&x["kind"]))).collect();
                let kinds: BTreeSet<&String> = chosen.iter().map(|c| &c.2).collect();
                let items: Vec<Value> = chosen.iter().map(|(f, l, _)| json!({"file": f, "line": l})).collect();
                if chosen.is_empty() {
                    return Err(app_err(msg("Nothing is selected to fix.", &[])));
                }
                let kinds = kinds.into_iter().map(|k| json!(k)).collect();
                (items, kinds, Some(chosen))
            }
            None => (items.to_vec(), kinds.to_vec(), None),
        };
        let plan = self.fix_preview(&items, file, &kinds)?;
        let plan: Vec<Value> = plan["items"].as_array().cloned().unwrap_or_default();
        let skip_of = |p: &Value| p.get("skip").filter(|s| crate::util::truthy(s)).cloned();
        let todo: Vec<&Value> = plan
            .iter()
            .filter(|p| {
                skip_of(p).is_none() && chosen.as_ref().is_none_or(|c| c.contains(&(py_str(&p["file"]), p["line"].as_i64().unwrap_or(0), py_str(&p["kind"]))))
            })
            .collect();
        let mut skipped: Vec<Value> = plan.iter().filter_map(|p| skip_of(p).map(|s| json!([p["file"], p["line"], s]))).collect();
        if todo.is_empty() {
            return Err(app_err(msg("Nothing to fix.", &[])));
        }
        let (mut fixed, mut backups): (usize, Vec<String>) = (0, Vec::new());
        let files: BTreeSet<String> = todo.iter().map(|p| py_str(&p["file"])).collect();
        self.snapshot_for_bulk(todo.len(), files.len());
        let outcome: Res<()> = (|| {
            let speaker: Vec<Planned> = todo
                .iter()
                .filter(|p| p["kind"] == "speaker")
                .map(|p| Planned {
                    file: py_str(&p["file"]),
                    line: p["line"].as_u64().unwrap_or(0) as usize,
                    before: py_str(&p["before"]),
                    after: py_str(&p["after"]),
                    skip: None,
                })
                .collect();
            if !speaker.is_empty() {
                let clock = &*self.clock;
                let o = self.open.as_mut().unwrap();
                let res = fixes::apply(&o.project, &o.pack, &o.local_dir, &speaker, clock).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
                fixed += res.fixed;
                backups.extend(res.backups);
                skipped.extend(res.skipped.iter().map(|(f, l, m)| json!([f, l, m.wire()])));
                let _ = o.project.refresh(true, clock);
                o.val_cache.clear();
            }
            let texts: BTreeSet<(String, i64, String)> =
                todo.iter().filter(|p| p["kind"] != "speaker").map(|p| (py_str(&p["file"]), p["line"].as_i64().unwrap_or(0), py_str(&p["kind"]))).collect();
            if !texts.is_empty() {
                let mut by_line: BTreeMap<(String, i64), BTreeSet<String>> = BTreeMap::new();
                for (f, l, k) in &texts {
                    by_line.entry((f.clone(), *l)).or_default().insert(k.clone());
                }
                let spell = self.spell();
                let mut reqs: Vec<Value> = Vec::new();
                {
                    let o = self.o()?;
                    for ((f, l), ks) in &by_line {
                        let e: Option<&Entry> = if o.project.files.contains_key(f) { o.project.entries(f).iter().find(|x| x.line as i64 == *l) } else { None };
                        let Some(e) = e.filter(|e| e.source.is_some()) else { continue };
                        let src = e.source.as_deref().unwrap_or("");
                        let issues: Vec<Value> = match (&spell, ks.contains("typo")) {
                            (Some(s), true) => {
                                use validator::SpellCheck;
                                s.check(src, &e.target)
                                    .iter()
                                    .map(|i| json!({"word": i.extra.get("word"), "better": i.extra.get("better"), "typo_kind": i.extra.get("typo_kind").cloned().unwrap_or(json!(""))}))
                                    .collect()
                            }
                            _ => Vec::new(),
                        };
                        reqs.push(json!({"file": f, "line": l, "source": src, "target": e.target, "kinds": ks, "issues": issues}));
                    }
                }
                let chain = self.ext_call("fixes.safe", "chain", &json!({"items": reqs}))?;
                let picks: Vec<(String, usize, String)> = chain["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| (py_str(&c["file"]), c["line"].as_u64().unwrap_or(0) as usize, py_str(&c["text"])))
                    .collect();
                let kinds_txt = texts.iter().map(|t| t.2.clone()).collect::<BTreeSet<_>>().iter().map(|k| fix_label(k)).collect::<Vec<_>>().join(", ");
                let label = Message::new(&format!("Opravy textu ({kinds_txt})"), None, Map::new());
                let clock = &*self.clock;
                let o = self.open.as_mut().unwrap();
                let res = fixes::apply_edits(&o.project, &o.pack, &o.local_dir, &picks, &label, clock).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
                let details: Vec<Value> = res.to_json()["details"].as_array().cloned().unwrap_or_default();
                self.log_text(&details, "fix");
                fixed += res.changed;
                backups.extend(res.backups);
            }
            Ok(())
        })();
        {
            let clock = &*self.clock;
            let o = self.open.as_mut().unwrap();
            let _ = o.project.refresh(true, clock);
            o.val_cache.clear();
        }
        outcome?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let mut out = self.changed("fix")?;
        out.insert("fixed".into(), json!(fixed));
        out.insert("skipped".into(), Value::Array(skipped));
        out.insert("backups".into(), json!(backups));
        Ok(Value::Object(out))
    }

    // ------------------------------------------------------------ find and replace in translations
    /// `(items, matches, truncated)` of a search; the regex mode asks the Pro pack to do the substitution on the plain texts.
    #[allow(clippy::too_many_arguments)]
    fn replace_plan(
        &mut self,
        find: &str,
        replace: &str,
        mode: &str,
        case: bool,
        files: &[Value],
        protect: bool,
        limit: usize,
    ) -> Res<(Vec<Value>, usize, bool)> {
        let rep = if mode != "regex" { Some(textedit::make_replacer(find, replace, mode, case).map_err(|m| ApiError::App(Text::new_msg(m)))?) } else { None };
        let files: Vec<String> = files.iter().map(py_str).collect();
        {
            let o = self.o()?;
            if let Some(u) = files.iter().find(|f| !o.project.files.contains_key(*f)) {
                return Err(app_err(msg("Unknown file: {name}", &[("name", json!(u))])));
            }
        }
        let rels: Vec<String> = {
            let mut v: Vec<String> = if files.is_empty() { self.o()?.project.files.keys().cloned().collect() } else { files };
            v.sort_by_key(|r| r.to_lowercase());
            v
        };
        let candidates: Vec<(String, Entry)> = {
            let o = self.o()?;
            rels.iter()
                .flat_map(|rel| o.project.entries(rel).iter().map(move |e| (rel, e)))
                .filter(|(rel, e)| !e.target.is_empty() && !(e.pending() && !e.category.is_empty() && !o.project.is_translated(&o.store, rel, e)))
                .map(|(rel, e)| (rel.clone(), e.clone()))
                .collect()
        };
        let mut done: Option<std::vec::IntoIter<(String, usize)>> = None;
        if rep.is_none() {
            let payload = json!({"pattern": find, "replace": replace, "case": case, "protect": protect, "texts": candidates.iter().map(|c| c.1.target.clone()).collect::<Vec<_>>()});
            let res = self.ext_call("replace.regex", "substitute", &payload)?;
            let list: Vec<(String, usize)> =
                res["results"].as_array().into_iter().flatten().map(|r| (py_str(&r["new"]), r["n"].as_u64().unwrap_or(0) as usize)).collect();
            done = Some(list.into_iter());
        }
        let (mut items, mut matches, mut truncated) = (Vec::new(), 0usize, false);
        for (rel, e) in &candidates {
            let (new, n) = match (&rep, done.as_mut()) {
                (Some(r), _) => r.apply(&e.target, protect),
                (None, Some(it)) => it.next().unwrap_or((e.target.clone(), 0)),
                (None, None) => (e.target.clone(), 0),
            };
            if n == 0 || new == e.target {
                continue;
            }
            let skip = match fixes::check_literal(&new) {
                Ok(()) => String::new(),
                Err(err) => err.0.text(),
            };
            if items.len() >= limit {
                truncated = true;
                break;
            }
            matches += n;
            items.push(json!({"file": rel, "line": e.line, "who": e.who, "source": e.source, "before": e.target, "after": new, "count": n, "skip": skip}));
        }
        Ok((items, matches, truncated))
    }

    /// Lines of already translated text that the search would change, with the new text. Nothing is written.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_preview(&mut self, find: &Value, replace: &Value, mode: &str, case: bool, files: &[Value], protect: bool, limit: i64) -> Res<Value> {
        self.need_project()?;
        if mode == "regex" {
            self.gate("replace.regex")?;
        }
        let (items, matches, truncated) = self.replace_plan(&py_str(find), &py_str(replace), mode, case, files, protect, limit.clamp(1, 20000) as usize)?;
        let applicable = items.iter().filter(|i| i["skip"] == "").count();
        Ok(json!({"items": items, "total": items.len(), "matches": matches, "truncated": truncated, "applicable": applicable}))
    }

    /// Write the replacement into the ticked lines (one undoable step, backup of every touched file).
    #[allow(clippy::too_many_arguments)]
    pub fn replace_apply(&mut self, selections: &[Value], find: &Value, replace: &Value, mode: &str, case: bool, files: &[Value], protect: bool) -> Res<Value> {
        self.need_project()?;
        if mode == "regex" {
            self.gate("replace.regex")?;
        }
        let chosen: BTreeSet<(String, i64)> = selections.iter().map(file_line).collect();
        if chosen.is_empty() {
            return Err(app_err(msg("No line is selected.", &[])));
        }
        let (items, _m, _t) = self.replace_plan(&py_str(find), &py_str(replace), mode, case, files, protect, 20000)?;
        let edits: Vec<(String, usize, String)> = items
            .iter()
            .filter(|i| i["skip"] == "" && chosen.contains(&(py_str(&i["file"]), i["line"].as_i64().unwrap_or(0))))
            .map(|i| (py_str(&i["file"]), i["line"].as_u64().unwrap_or(0) as usize, py_str(&i["after"])))
            .collect();
        if edits.is_empty() {
            return Err(app_err(msg("The selected lines changed in the meantime – show the preview again.", &[])));
        }
        let (a, b): (String, String) = (py_str(find).chars().take(30).collect(), py_str(replace).chars().take(30).collect());
        let label = msg("Replacing “{a}” → “{b}”", &[("a", json!(a)), ("b", json!(b))]);
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
        self.log_text(&details, "replace");
        let mut out = self.changed("replace")?;
        out.insert("changed".into(), json!(res.changed));
        out.insert("files".into(), json!(res.files));
        out.insert("backups".into(), json!(res.backups));
        Ok(Value::Object(out))
    }

    pub fn fix_last(&self) -> Res<Value> {
        let o = self.o()?;
        Ok(json!({"last": fixes::last(&o.local_dir)}))
    }

    /// Restore the files changed by the last fix from their backups.
    pub fn fix_undo(&mut self) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let before: Vec<(String, Vec<Entry>)> = o.project.files.keys().map(|rel| (rel.clone(), o.project.entries(rel).to_vec())).collect();
        let res = fixes::undo(&o.project, &o.local_dir, clock);
        let _ = o.project.refresh(true, clock);
        o.val_cache.clear();
        let res = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let restored: Vec<String> = res["restored"].as_array().into_iter().flatten().map(py_str).collect();
        let mut logged: Vec<Value> = Vec::new();
        for rel in &restored {
            if let (Some((_, old)), true) = (before.iter().find(|(r, _)| r == rel), o.project.files.contains_key(rel)) {
                for c in texthistory::diff_files(old, o.project.entries(rel)) {
                    logged.push(json!({"file": rel, "key": [c.key.0, c.key.1], "old": c.old, "new": c.new}));
                }
            }
        }
        self.log_text(&logged, "undo");
        let mut out = self.changed("fix-undo")?;
        if let Value::Object(r) = res {
            out.extend(r);
        }
        Ok(Value::Object(out))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_fix_last(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.fix_last()?)
}

pub fn op_fix_preview(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.fix_preview(a.l("items"), a.s("file"), a.l("kinds"))?)
}

pub fn op_fix_apply(app: &mut App, a: &Args) -> Res<Answer> {
    let sel = a.raw().get("selections").and_then(Value::as_array).cloned();
    plain(app.fix_apply(a.l("items"), a.s("file"), a.l("kinds"), sel.as_ref())?)
}

pub fn op_fix_undo(app: &mut App, _a: &Args) -> Res<Answer> {
    plain(app.fix_undo()?)
}

pub fn op_replace_preview(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.replace_preview(a.a("find"), a.a("replace"), a.s("mode"), a.f("case"), a.l("files"), a.f("protect"), 2000)?)
}

pub fn op_replace_apply(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.replace_apply(a.l("selections"), a.a("find"), a.a("replace"), a.s("mode"), a.f("case"), a.l("files"), a.f("protect"))?)
}
