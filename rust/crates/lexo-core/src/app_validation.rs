//! Port of `reference/python/core/lexo/app_validation.py`: findings of the checks (validator, spelling, ignored findings), the overview and the estimates.
use crate::api::{ok, Answer, Args};
use crate::app::{App, CachedIssues, Found};
use crate::app_base::{app_err, Res};
use crate::fsutil::text_matcher;
use crate::history;
use crate::messages::{msg, py_str};
use crate::project::{file_status, totals_of, STATUS_ORDER};
use crate::pyfmt::{json_dumps, round_digits};
use crate::rpy::Entry;
use crate::spelling::{words_of, Spelling};
use crate::store::{ep_states, text_hash, Kind, Label, Mark};
use crate::util::{norm_ws, sha1_hex, truthy};
use crate::validator::{self, Checks, Issue};
use crate::{export, rpy};
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// The text a finding is bound to: the original, or the translation of a line that has none.
fn source_or_target(e: &Entry) -> String {
    norm_ws(e.source.as_deref().unwrap_or(&e.target))
}

impl App {
    // ------------------------------------------------------------ validation
    pub fn length_cfg(&self) -> Value {
        self.config.get("validation.length").cloned().unwrap_or_else(|| json!({}))
    }

    pub fn textbox_cfg(&self) -> Value {
        self.config.get("validation.textbox").cloned().unwrap_or_else(|| json!({}))
    }

    /// Glossary terms `(English, Czech)` used by the term-consistency check.
    pub fn terms(&self) -> Vec<(String, String)> {
        match self.open.as_ref() {
            Some(o) => o.store.glossary.iter().filter(|(_, g)| g.t == "term" && !g.n.is_empty()).map(|(c, g)| (c.clone(), g.n.clone())).collect(),
            None => Vec::new(),
        }
    }

    /// Spelling hints built from the whole project's translated text; rebuilt when any file or the glossary changes.
    pub fn spell(&mut self) -> Option<Rc<Spelling>> {
        if !self.cfg_bool("validation.spelling", true) {
            return None;
        }
        let o = self.open.as_mut()?;
        let mut files: Vec<(&String, &str)> = o.project.files.iter().map(|(rel, fi)| (rel, fi.parsed.digest.as_str())).collect();
        files.sort();
        let mut glossary: Vec<(&String, &String)> = o.store.glossary.iter().map(|(c, g)| (c, &g.n)).collect();
        glossary.sort();
        let key = sha1_hex(format!("{files:?}|{glossary:?}").as_bytes());
        if let Some((k, s)) = &o.spell_cache {
            if *k == key {
                return Some(s.clone());
            }
        }
        let mut known: Vec<String> = Vec::new();
        for (c, g) in &o.store.glossary {
            known.extend(words_of(c));
            known.extend(words_of(&g.n));
        }
        let targets: Vec<&str> = o.project.files.values().flat_map(|fi| fi.parsed.entries.iter().filter(|e| !e.pending()).map(|e| e.target.as_str())).collect();
        let obj = Rc::new(Spelling::build(&targets, &known));
        o.spell_cache = Some((key, obj.clone()));
        Some(obj)
    }

    /// All findings of a file (cached until the file, the settings or the glossary change): `(line, finding, entry index)`.
    pub fn raw_issues(&mut self, rel: &str) -> Res<Rc<Vec<Found>>> {
        let (length, textbox, terms) = (self.length_cfg(), self.textbox_cfg(), self.terms());
        let spell = self.spell();
        let o = self.o_mut()?;
        let Some(fi) = o.project.files.get(rel) else { return Err(crate::app::unknown_file()) };
        let sig = json_dumps(&json!([length, terms, textbox, spell.as_ref().map_or("", |s| s.sig.as_str())]), true);
        if let Some(hit) = o.val_cache.get(rel) {
            if hit.digest == fi.parsed.digest && hit.sig == sig {
                return Ok(hit.issues.clone());
            }
        }
        let checks =
            Checks { length_cfg: Some(&length), terms: &terms, spell: spell.as_deref().map(|s| s as &dyn validator::SpellCheck), textbox: Some(&textbox) };
        let res = Rc::new(validator::check_parsed(&fi.parsed, &o.pack, &checks));
        o.val_cache.insert(rel.to_string(), CachedIssues { digest: fi.parsed.digest.clone(), sig, issues: res.clone() });
        Ok(res)
    }

    // ------------------------------------------------------------ ignored findings
    /// The stored ignore of this finding if it still applies (same code, same translation text), else `None`.
    pub fn ignored_mark(&self, rel: &str, e: Option<&Entry>, iss: &Issue) -> Option<Mark> {
        let e = e?;
        let o = self.open.as_ref()?;
        let marks = o.store.ignored.get(rel).filter(|m| !m.is_empty())?;
        let m = marks.get(&(e.block.clone(), source_or_target(e), iss.code.clone()))?;
        (m.get("h").and_then(Value::as_str) == Some(text_hash(&e.target).as_str())).then(|| m.clone())
    }

    /// Findings of a file; by default without the ones the translator ignored (`ignored`: only those).
    pub fn issues(&mut self, rel: &str, ignored: bool) -> Res<Vec<Found>> {
        let raw = self.raw_issues(rel)?;
        let has_marks = self.o()?.store.ignored.get(rel).is_some_and(|m| !m.is_empty());
        if !has_marks {
            return Ok(if ignored { Vec::new() } else { raw.to_vec() });
        }
        let entries = &self.o()?.project.files[rel].parsed.entries;
        Ok(raw.iter().filter(|r| self.ignored_mark(rel, r.2.map(|i| &entries[i]), &r.1).is_some() == ignored).cloned().collect())
    }

    /// Ignore (`on`) or stop ignoring findings. Items: `{file, line, code}` (or `{file, key, code}` to stop ignoring a finding whose line no longer exists). An ignore is bound to
    /// the current translation text: when the text changes the finding shows up again. One undo step. Returns `{changed, skipped}`.
    pub fn ignore_set(&mut self, items: &[Value], on: bool, note: &Value) -> Res<Value> {
        self.need_project()?;
        // (file, key, translation of the line)
        let mut plan: Vec<(String, (String, String, String), String)> = Vec::new();
        let mut skipped: Vec<Value> = Vec::new();
        for it in items {
            let rel = it.get("file").and_then(Value::as_str).unwrap_or("").to_string();
            let code = it.get("code").map_or(String::new(), py_str);
            let known = self.o()?.project.files.contains_key(&rel);
            if !known {
                skipped.push(json!([it.get("file").cloned().unwrap_or(Value::Null), msg("unknown file", &[]).wire()]));
                continue;
            }
            let line = it.get("line").filter(|v| truthy(v)).and_then(|v| v.as_i64().or_else(|| py_str(v).parse().ok()));
            #[allow(clippy::type_complexity)]
            let (key, entry): (Option<(String, String, String)>, Option<(usize, Entry)>) = {
                let ents = self.o()?.project.entries(&rel);
                if let Some(line) = line {
                    let found = ents.iter().position(|x| x.line as i64 == line);
                    (found.map(|i| (ents[i].block.clone(), source_or_target(&ents[i]), code.clone())), found.map(|i| (i, ents[i].clone())))
                } else {
                    let k = it.get("key").filter(|v| truthy(v)).and_then(Value::as_array).cloned().unwrap_or_else(|| vec![json!(""), json!("")]);
                    let key = (py_str(&k[0]), norm_ws(&py_str(k.get(1).unwrap_or(&json!("")))), code.clone());
                    let found = ents.iter().position(|x| (x.block.clone(), source_or_target(x)) == (key.0.clone(), key.1.clone()));
                    (Some(key), found.map(|i| (i, ents[i].clone())))
                }
            };
            let Some(key) = key else {
                skipped.push(json!([rel, msg("the line changed in the file – refresh the files", &[]).wire()]));
                continue;
            };
            if on {
                let exists = match &entry {
                    Some((idx, _)) => self.raw_issues(&rel)?.iter().any(|r| r.2 == Some(*idx) && r.1.code == code),
                    None => false,
                };
                if !exists {
                    skipped.push(json!([rel, msg("this finding no longer exists", &[]).wire()]));
                    continue;
                }
            }
            plan.push((rel, key, entry.map(|(_, e)| e.target).unwrap_or_default()));
        }
        let clean_note: String = py_strip_note(note);
        let today = self.clock.now_local().date().format("%Y-%m-%d").to_string();
        let mut changed = 0usize;
        {
            let clock = &*self.clock;
            let o = self.open.as_mut().unwrap();
            let label = if on { msg("Ignoring findings", &[]) } else { msg("Un-ignoring findings", &[]) };
            o.store.change(
                Label::Msg(label),
                &[Kind::Ignored],
                |st| {
                    for (rel, key, target) in &plan {
                        let marks = st.ignored.entry(rel.clone()).or_default();
                        if on {
                            let new: Mark = [("h", text_hash(target)), ("n", clean_note.clone()), ("d", today.clone())]
                                .into_iter()
                                .map(|(k, v)| (k.to_string(), json!(v)))
                                .collect();
                            if marks.get(key) != Some(&new) {
                                marks.insert(key.clone(), new);
                                changed += 1;
                            }
                        } else if marks.shift_remove(key).is_some() {
                            changed += 1;
                        }
                        if marks.is_empty() {
                            st.ignored.shift_remove(rel);
                        }
                    }
                },
                clock,
            )?;
        }
        let mut out = self.changed("ignore")?;
        out.insert("changed".into(), json!(changed));
        out.insert("skipped".into(), Value::Array(skipped));
        Ok(Value::Object(out))
    }

    /// Everything the translator chose to ignore, with its current state: active (still hidden), expired (the translation changed, the finding is shown again), resolved
    /// (the finding no longer occurs) or gone (the line no longer exists).
    pub fn ignored_list(&mut self) -> Res<Value> {
        self.need_project()?;
        let mut rels: Vec<String> = self.o()?.store.ignored.keys().cloned().collect();
        rels.sort_by_key(|r| r.to_lowercase());
        let mut out: Vec<Value> = Vec::new();
        for rel in rels {
            let mut marks: Vec<((String, String, String), Mark)> = self.o()?.store.ignored[&rel].iter().map(|(k, m)| (k.clone(), m.clone())).collect();
            marks.sort_by(|a, b| a.0.cmp(&b.0));
            let note = |m: &Mark, k: &str| m.get(k).cloned().unwrap_or(json!(""));
            if !self.o()?.project.files.contains_key(&rel) {
                for ((blk, src, code), m) in &marks {
                    out.push(json!({"file": rel, "line": 0, "key": [blk, src], "code": code, "note": note(m, "n"), "date": note(m, "d"), "state": "gone", "message": "", "severity": "",
                        "source": src, "target": ""}));
                }
                continue;
            }
            let ents: Vec<Entry> = self.o()?.project.entries(&rel).to_vec();
            let raw = self.raw_issues(&rel)?;
            for ((blk, src, code), m) in &marks {
                let idx = ents.iter().position(|x| (&x.block, &source_or_target(x)) == (blk, src));
                let e = idx.map(|i| &ents[i]);
                let mut rec = json!({"file": rel, "line": e.map_or(0, |e| e.line), "key": [blk, src], "code": code, "note": note(m, "n"), "date": note(m, "d"), "message": "", "severity": "",
                    "source": e.and_then(|e| e.source.clone()).unwrap_or_else(|| src.clone()), "target": e.map_or("", |e| e.target.as_str())});
                match e {
                    None => rec["state"] = json!("gone"),
                    Some(e) if m.get("h").and_then(Value::as_str) != Some(text_hash(&e.target).as_str()) => rec["state"] = json!("expired"),
                    Some(_) => {
                        let hit = raw.iter().find(|r| r.2 == idx && r.1.code == *code).map(|r| &r.1);
                        rec["state"] = json!(if hit.is_some() { "active" } else { "resolved" });
                        if let Some(h) = hit {
                            rec["message"] = h.message.wire();
                            rec["severity"] = json!(h.severity);
                        }
                    }
                }
                out.push(rec);
            }
        }
        let count = |s: &str| out.iter().filter(|r| r["state"] == s).count();
        let counts = json!({"active": count("active"), "expired": count("expired"), "resolved": count("resolved"), "gone": count("gone")});
        Ok(json!({"items": out, "counts": counts}))
    }

    /// The findings of every file (nothing when the checks are off).
    pub fn all_issues(&mut self) -> Res<Vec<(String, Vec<Found>)>> {
        if !self.cfg_bool("validation.enabled", true) {
            return Ok(Vec::new());
        }
        let rels: Vec<String> = self.o()?.project.files.keys().cloned().collect();
        let mut out = Vec::new();
        for rel in rels {
            let v = self.issues(&rel, false)?;
            out.push((rel, v));
        }
        Ok(out)
    }

    /// Duplicate `translate <lang> <id>` blocks across the project (Ren'Py rejects them): `(file, line, where the first one is)`.
    pub fn duplicate_ids(&self) -> Vec<(String, usize, String)> {
        let Some(o) = self.open.as_ref() else { return Vec::new() };
        let mut rels: Vec<&String> = o.project.files.keys().collect();
        rels.sort();
        let mut seen: IndexMap<(String, String), String> = IndexMap::new();
        let mut dup = Vec::new();
        for rel in rels {
            for (lang, ident, line) in &o.project.files[rel].parsed.block_ids {
                let k = (lang.clone(), ident.clone());
                match seen.get(&k) {
                    Some(first) => dup.push((rel.clone(), *line, first.clone())),
                    None => {
                        seen.insert(k, format!("{rel}:{line}"));
                    }
                }
            }
        }
        dup
    }

    /// `(counts, per file)`: how many findings of each severity (and how many are about length, how many are ignored).
    #[allow(clippy::type_complexity)]
    pub fn validation_summary(&mut self) -> Res<(Map<String, Value>, IndexMap<String, [i64; 3]>)> {
        let mut counts: IndexMap<&str, i64> = [("error", 0), ("warning", 0), ("info", 0), ("length", 0)].into_iter().collect();
        let mut per_file: IndexMap<String, [i64; 3]> = IndexMap::new();
        let slot = |sev: &str| match sev {
            "error" => 0,
            "warning" => 1,
            _ => 2,
        };
        for (rel, items) in self.all_issues()? {
            let mut c = [0i64; 3];
            for (_line, iss, _e) in &items {
                if iss.category == "length" {
                    *counts.get_mut("length").unwrap() += 1;
                }
                *counts.get_mut(iss.severity).unwrap() += 1;
                c[slot(iss.severity)] += 1;
            }
            per_file.insert(rel, c);
        }
        let show_info = self.cfg_bool("validation.show_info", false);
        let enabled = self.cfg_bool("validation.enabled", true);
        let mut ignored = 0i64;
        if enabled {
            let rels: Vec<String> = self.o()?.project.files.keys().cloned().collect();
            for rel in rels {
                ignored += self.issues(&rel, true)?.iter().filter(|(_, i, _)| i.severity != "info" || show_info).count() as i64;
            }
        }
        let dups = if enabled { self.duplicate_ids() } else { Vec::new() };
        *counts.get_mut("error").unwrap() += dups.len() as i64;
        for (rel, _, _) in &dups {
            per_file.entry(rel.clone()).or_insert([0; 3])[0] += 1;
        }
        let mut out: Map<String, Value> = counts.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        out.insert("ignored".into(), json!(ignored));
        Ok((out, per_file))
    }

    /// Findings. `ignored`: "hide" (default) | "only" (just the ignored ones) | "all". `group` = "line": one item per line with all its (matching) findings in `issues`;
    /// `total` counts lines.
    #[allow(clippy::too_many_arguments)]
    pub fn validation_list(
        &mut self,
        severity: &str,
        category: &str,
        file: &str,
        q: &str,
        offset: i64,
        limit: i64,
        ignored: &str,
        group: &str,
        word: bool,
    ) -> Res<Value> {
        self.need_project()?;
        let matcher = text_matcher(q, word);
        let show_info = self.cfg_bool("validation.show_info", false);
        let enabled = self.cfg_bool("validation.enabled", true);
        let mut sev_counts = [("error", 0i64), ("warning", 0), ("info", 0)];
        let mut out: Vec<Map<String, Value>> = Vec::new();
        let mut rels: Vec<String> = self.o()?.project.files.keys().cloned().collect();
        rels.sort();
        for rel in if enabled { rels } else { Vec::new() } {
            if !file.is_empty() && rel != file {
                continue;
            }
            let mut rows: Vec<(Found, bool)> = Vec::new();
            if ignored == "hide" || ignored == "all" {
                rows.extend(self.issues(&rel, false)?.into_iter().map(|r| (r, false)));
            }
            if ignored == "only" || ignored == "all" {
                rows.extend(self.issues(&rel, true)?.into_iter().map(|r| (r, true)));
            }
            let ents: Vec<Entry> = self.o()?.project.entries(&rel).to_vec();
            for ((line, iss, idx), is_ign) in rows {
                let e = idx.map(|i| &ents[i]);
                if !category.is_empty() && iss.category != category {
                    continue;
                }
                if let Some(m) = &matcher {
                    if !m(&format!("{} {} {}", iss.message.text(), rel, e.map_or("", |e| e.target.as_str()))) {
                        continue;
                    }
                }
                sev_counts.iter_mut().find(|(k, _)| *k == iss.severity).unwrap().1 += 1;
                if !severity.is_empty() && iss.severity != severity {
                    continue;
                }
                if severity.is_empty() && iss.severity == "info" && !show_info {
                    continue;
                }
                let note = if is_ign { self.ignored_mark(&rel, e, &iss).and_then(|m| m.get("n").cloned()).unwrap_or(json!("")) } else { json!("") };
                let mut r = Map::new();
                r.insert("file".into(), json!(rel));
                r.insert("line".into(), json!(line));
                r.insert("severity".into(), json!(iss.severity));
                r.insert("code".into(), json!(iss.code));
                r.insert("category".into(), json!(iss.category));
                r.insert("message".into(), iss.message.wire());
                r.insert("source".into(), json!(e.and_then(|e| e.source.clone()).unwrap_or_default()));
                r.insert("target".into(), json!(e.map_or("", |e| e.target.as_str())));
                r.insert("who".into(), json!(e.map_or("", |e| e.who.as_str())));
                r.insert("extra".into(), Value::Object(iss.extra.clone()));
                r.insert("src_who".into(), json!(e.map_or("", |e| e.src_who.as_str())));
                r.insert("ignored".into(), json!(is_ign));
                r.insert("ignore_note".into(), note);
                out.push(r);
            }
        }
        if category.is_empty() && q.is_empty() && ignored != "only" {
            for (rel, line, first) in self.duplicate_ids() {
                if file.is_empty() || rel == file {
                    sev_counts[0].1 += 1;
                    if !severity.is_empty() && severity != "error" {
                        continue;
                    }
                    let mut r = Map::new();
                    r.insert("file".into(), json!(rel));
                    r.insert("line".into(), json!(line));
                    r.insert("severity".into(), json!("error"));
                    r.insert("code".into(), json!("duplicate-id"));
                    r.insert("category".into(), json!("structure"));
                    r.insert(
                        "message".into(),
                        msg("A translate block with the same identifier already exists ({first}); Ren'Py will reject it.", &[("first", json!(first))]).wire(),
                    );
                    for k in ["source", "target", "who"] {
                        r.insert(k.into(), json!(""));
                    }
                    r.insert("extra".into(), json!({}));
                    out.push(r);
                }
            }
        }
        let order = |r: &Map<String, Value>| match r["severity"].as_str().unwrap_or("") {
            "error" => 0,
            "warning" => 1,
            _ => 2,
        };
        out.sort_by_key(|r| (order(r), r["file"].as_str().unwrap_or("").to_lowercase(), r["line"].as_i64().unwrap_or(0)));
        let sev = json!({"error": sev_counts[0].1, "warning": sev_counts[1].1, "info": sev_counts[2].1});
        let (offset, limit) = (offset.max(0) as usize, limit.max(0) as usize);
        if group == "line" {
            let mut lines: IndexMap<(String, i64), Map<String, Value>> = IndexMap::new();
            for r in &out {
                // `out` is sorted by severity, so the first finding is the worst
                let k = (r["file"].as_str().unwrap_or("").to_string(), r["line"].as_i64().unwrap_or(0));
                let g = lines.entry(k).or_insert_with(|| {
                    let mut g = Map::new();
                    for key in ["file", "line", "severity", "source", "target", "who"] {
                        g.insert(key.into(), r[key].clone());
                    }
                    g.insert("src_who".into(), r.get("src_who").cloned().unwrap_or(json!("")));
                    g.insert("issues".into(), json!([]));
                    g
                });
                let issue: Map<String, Value> = ["severity", "code", "category", "message", "extra", "ignored", "ignore_note"]
                    .iter()
                    .filter_map(|k| r.get(*k).map(|v| (k.to_string(), v.clone())))
                    .collect();
                g["issues"].as_array_mut().unwrap().push(Value::Object(issue));
            }
            let mut grouped: Vec<Map<String, Value>> = lines.into_values().collect();
            grouped.sort_by_key(|g| (order(g), g["file"].as_str().unwrap_or("").to_lowercase(), g["line"].as_i64().unwrap_or(0)));
            let page: Vec<Value> = grouped.iter().skip(offset).take(limit).map(|g| Value::Object(g.clone())).collect();
            return Ok(json!({"total": grouped.len(), "findings": out.len(), "sev_counts": sev, "items": page}));
        }
        let page: Vec<Value> = out.iter().skip(offset).take(limit).map(|g| Value::Object(g.clone())).collect();
        Ok(json!({"total": out.len(), "sev_counts": sev, "items": page}))
    }

    // ------------------------------------------------------------ overview / files
    pub fn estimate(&self) -> Res<Value> {
        let o = self.o()?;
        let rows = o.project.rows(&o.store);
        let (t_done, total, ..) = totals_of(&rows);
        Ok(history::estimate(&o.history.points(), t_done, total, self.clock.now_local().date()).to_json())
    }

    /// Weekly goals and this week's progress (see `history::goals`).
    pub fn goals(&self) -> Res<Value> {
        let o = self.o()?;
        let rows = o.project.rows(&o.store);
        let (t_done, total, c_done, ..) = totals_of(&rows);
        Ok(history::goals(&o.history.points(), (total - t_done).max(0), (total - c_done).max(0), self.config.get("goals"), self.clock.now_local().date()))
    }

    /// The files with the most findings: `(file, errors, warnings, lines, score)`, worst first.
    fn worst_files(&self, per_file: &IndexMap<String, [i64; 3]>, limit: usize) -> Res<Vec<Value>> {
        let o = self.o()?;
        let totals: IndexMap<String, i64> = o.project.rows(&o.store).into_iter().map(|r| (r.name, r.total)).collect();
        let mut out: Vec<(f64, i64, String, Value)> = Vec::new();
        for (rel, c) in per_file {
            let problems = c[0] * 3 + c[1];
            let Some(total) = totals.get(rel) else { continue };
            if problems == 0 {
                continue;
            }
            let score = round_digits(problems as f64 / (*total).max(1) as f64 * 100.0, 1);
            out.push((score, c[0], rel.to_lowercase(), json!({"name": rel, "errors": c[0], "warnings": c[1], "lines": total, "score": score})));
        }
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        Ok(out.into_iter().take(limit).map(|r| r.3).collect())
    }

    pub fn overview(&mut self) -> Res<Value> {
        self.need_project()?;
        let (vcounts, per_file) = self.validation_summary()?;
        let est_now = self.estimate()?;
        let goals = self.goals()?;
        let worst = self.worst_files(&per_file, 10)?;
        let lock_lost = match self.open.as_mut() {
            Some(o) => o.plock.as_mut().is_some_and(|l| !l.check()),
            None => false,
        };
        let o = self.o()?;
        let rows = o.project.rows(&o.store);
        let (t_done, total, c_done, tpct, cpct) = totals_of(&rows);
        let mut counts: Map<String, Value> = STATUS_ORDER.iter().map(|k| (k.to_string(), json!(0))).collect();
        for r in &rows {
            let k = file_status(r.done, r.todo, r.total, r.corr);
            counts[k] = json!(counts[k].as_i64().unwrap_or(0) + 1);
        }
        let approved: usize = o.store.approved.values().map(|v| v.len()).sum();
        let (mut marked, mut flagged, mut stale) = (0i64, 0i64, 0i64);
        for rel in o.store.proof.keys() {
            if o.project.files.contains_key(rel) {
                let (ok_, fix) = o.project.proof_counts(&o.store, rel);
                marked += ok_;
                flagged += fix;
                stale += o.project.proof_stale(&o.store, rel);
            }
        }
        let mut stats = export::stats_for(&rows, &o.store, &o.store.layout);
        let pace = est_now.get("pace_per_day").and_then(Value::as_f64).filter(|p| *p != 0.0);
        if let Some(eps) = stats.get_mut("episodes").and_then(Value::as_array_mut) {
            for e in eps {
                // how long an episode would take at the current pace
                let left = e["translation"]["total"].as_i64().unwrap_or(0) - e["translation"]["done"].as_i64().unwrap_or(0);
                e["eta_days"] = match pace {
                    Some(p) if left > 0 => json!((-(-(left as f64) / p).floor()) as i64),
                    _ => Value::Null,
                };
            }
        }
        let pts = o.history.points();
        let mut recent: Vec<Value> = Vec::new();
        let mut prev: Option<&history::Point> = None;
        for p in pts.iter().skip(pts.len().saturating_sub(40)) {
            if let Some(q) = prev {
                if p.1 != q.1 || p.3 != q.3 {
                    recent.push(
                        json!({"at": p.0.format(history::STAMP_FORMAT).to_string(), "translated": p.1 - q.1, "proofread": p.3 - q.3, "total_translated": p.1}),
                    );
                }
            }
            prev = Some(p);
        }
        let recent: Vec<Value> = recent.iter().rev().take(8).cloned().collect();
        let labels = o.store.undo_label_values();
        let undo: Vec<Value> = labels.iter().rev().take(5).cloned().collect();
        let mut unsaved: Vec<&str> = o.store.unsaved.iter().copied().collect();
        unsaved.sort();
        let layout = &o.store.layout;
        let group_names: Map<String, Value> =
            layout.folders.iter().enumerate().map(|(i, f)| ((i + 1).to_string(), json!(f.rsplit('/').next().unwrap_or("")))).collect();
        let mut skipped: Vec<&String> = o.project.skip().iter().collect();
        skipped.sort();
        Ok(json!({
            "project": {"path": o.project.root.to_string_lossy(), "data_dir": o.data_dir.to_string_lossy(), "local_dir": o.local_dir.to_string_lossy(), "files": rows.len(), "skipped": skipped},
            "totals": {"done": t_done, "total": total, "remaining": total - t_done, "percent": round_digits(tpct, 1), "proofread": c_done, "proofread_percent": round_digits(cpct, 1),
                       "approved": approved, "proof_marked": marked, "proof_fix": flagged, "proof_stale": stale},
            "file_counts": counts,
            "seasons": stats["seasons"], "episodes": stats["episodes"], "labels": layout.labels, "groups_defined": !layout.active_groups().is_empty(), "group_names": group_names,
            "general": stats["general"], "unassigned": stats["unassigned"],
            "validation": vcounts, "worst_files": worst, "goals": goals, "estimate": est_now, "recent": recent, "undo": undo, "unsaved": unsaved,
            "last_save_error": self.last_save_error,
            "last_refresh": o.project.last_report.as_ref().map_or(Value::Null, |r| r.to_json()),
            "lock_lost": lock_lost, "loaded_at": o.project.loaded_at,
            "episode_states": ep_states().into_iter().map(|(k, l)| json!({"key": k, "label": l.wire()})).collect::<Vec<_>>(),
        }))
    }
}

/// `str(note or "").strip()[:300]`.
fn py_strip_note(note: &Value) -> String {
    let s = if truthy(note) { py_str(note) } else { String::new() };
    crate::util::py_strip(&s).chars().take(300).collect()
}

#[allow(dead_code)]
fn unused(_: &rpy::ParsedFile) -> crate::app_base::ApiError {
    app_err(msg("", &[]))
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_overview(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.overview()?)
}

pub fn op_validation(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.validation_list(a.s("severity"), a.s("category"), a.s("file"), a.s("q"), a.i("offset"), a.i("limit"), a.s("ignored"), a.s("group"), a.f("word"))?)
}

pub fn op_ignore_list(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.ignored_list()?)
}

pub fn op_ignore_set(app: &mut App, a: &Args) -> Res<Answer> {
    plain_value(app.ignore_set(a.l("items"), a.f("on"), a.a("note"))?)
}

fn plain_value(v: Value) -> Res<Answer> {
    Ok(Answer::Json(v))
}
