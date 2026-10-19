//! Port of `reference/python/core/lexo/app_entries.py`: files and lines: listing, detail, saving, context, scenes, text history of a line.
use crate::api::{ok, plain, Answer, Args};
use crate::app::{line_gone, unknown_file, App};
use crate::app_base::{app_err, ApiError, Res, PAGE_LIMIT};
use crate::app_game::{script_lines, split_ref};
use crate::fixes;
use crate::fsutil::{resolve, text_matcher};
use crate::gametools::below;
use crate::messages::{msg, Text};
use crate::project::{file_status, STATUS_ORDER};
use crate::rpy::{self, Entry};
use crate::store::{parse_value, Corr, Kind, Label};
use crate::util::norm_ws;
use crate::validator::{self, Checks};
use serde_json::{json, Map, Value};

fn kind_rank(category: &str) -> i32 {
    match category {
        rpy::K_EMPTY => 0,
        rpy::K_ENGLISH => 1,
        rpy::K_SAME => 2,
        rpy::K_SYMBOLS => 3,
        _ => 9,
    }
}

/// The text a finding is bound to: the original, or the translation of a line that has none.
fn source_or_target(e: &Entry) -> String {
    norm_ws(e.source.as_deref().unwrap_or(&e.target))
}

impl App {
    pub fn files_payload(&mut self) -> Res<Vec<Value>> {
        self.need_project()?;
        let (_, per_file) = self.validation_summary()?;
        let o = self.o()?;
        let mut out = Vec::new();
        for r in o.project.rows(&o.store) {
            let appr = o.project.counts(&o.store, &r.name).2;
            let st = file_status(r.done, r.todo, r.total, r.corr);
            let v = per_file.get(&r.name).copied().unwrap_or([0; 3]);
            let (p_ok, p_fix) = o.project.proof_counts(&o.store, &r.name);
            let order = STATUS_ORDER.iter().position(|s| *s == st).unwrap_or(0);
            out.push(json!({
                "name": r.name, "proof_marked": p_ok, "proof_fix": p_fix, "assign": o.store.assign.get(&r.name).cloned().unwrap_or_default(), "total": r.total, "done": r.done,
                "auto": r.auto, "todo": r.todo, "corr": r.corr, "approved": appr, "status": st, "order": order, "errors": v[0], "warnings": v[1],
                "proof_stale": o.project.proof_stale(&o.store, &r.name),
            }));
        }
        Ok(out)
    }

    // ------------------------------------------------------------ entries
    pub fn entry_row(&self, rel: &str, e: &Entry) -> &'static str {
        let Some(o) = self.open.as_ref() else { return "todo" };
        if !e.pending() {
            "translated"
        } else if o.store.approved.get(rel).is_some_and(|a| a.contains(&e.key())) {
            "approved"
        } else {
            "todo"
        }
    }

    /// The proof state of a line that counts as translated, else `""`.
    fn proof_if_translated(&self, rel: &str, e: &Entry) -> &'static str {
        let o = self.open.as_ref().unwrap();
        if o.project.is_translated(&o.store, rel, e) {
            o.project.proof_state(&o.store, rel, e)
        } else {
            ""
        }
    }

    /// The code of a speaker: `mc happy` -> `mc` (the first word, the rest are image attributes).
    pub fn speaker_code(who: &str) -> String {
        crate::util::py_split_ws(who).first().map(|s| s.to_string()).unwrap_or_default()
    }

    /// Paged, filtered list of translation units (server side, so 45k lines stay fast).
    #[allow(clippy::too_many_arguments)]
    pub fn entries(
        &mut self,
        file: &str,
        status: &str,
        kind: &str,
        q: &str,
        issues: &str,
        sort: &str,
        offset: i64,
        limit: i64,
        around: i64,
        unit: &str,
        word: bool,
    ) -> Res<Value> {
        self.need_project()?;
        let limit = limit.clamp(1, PAGE_LIMIT) as usize;
        let matcher = text_matcher(q, word);
        let mut files: Vec<String> = if file.is_empty() {
            let mut v: Vec<String> = self.o()?.project.files.keys().cloned().collect();
            v.sort_by_key(|r| r.to_lowercase());
            v
        } else {
            vec![file.to_string()]
        };
        if !file.is_empty() && !self.o()?.project.files.contains_key(file) {
            return Err(unknown_file());
        }
        let show_info = self.cfg_bool("validation.show_info", false);
        let mut iss_by: std::collections::HashMap<(String, usize), Vec<validator::Issue>> = std::collections::HashMap::new();
        if self.cfg_bool("validation.enabled", true) {
            for rel in &files {
                for (line, iss, _e) in self.issues(rel, false)? {
                    iss_by.entry((rel.clone(), line)).or_default().push(iss);
                }
            }
        }
        let mut rows: Vec<(String, Entry, &'static str, Vec<validator::Issue>, &'static str)> = Vec::new();
        let mut counts: Vec<(&str, i64)> = ["all", "todo", "approved", "translated", "review", "proofed", "fix", "stale"].iter().map(|k| (*k, 0)).collect();
        for rel in std::mem::take(&mut files) {
            let o = self.o()?;
            for e in o.project.entries(&rel) {
                let st = self.entry_row(&rel, e);
                let pf = if st != "todo" { o.project.proof_state(&o.store, &rel, e) } else { "" };
                let member = [
                    ("all", true),
                    ("todo", st == "todo"),
                    ("approved", st == "approved"),
                    ("translated", st == "translated"),
                    ("review", st != "todo" && pf != "ok" && pf != "fix"),
                    ("proofed", pf == "ok"),
                    ("fix", pf == "fix"),
                    ("stale", pf == "stale"),
                ];
                if !kind.is_empty() && e.category != kind {
                    continue;
                }
                if !unit.is_empty() && e.unit != unit {
                    continue;
                }
                let iss = iss_by.get(&(rel.clone(), e.line)).cloned().unwrap_or_default();
                let shown: Vec<validator::Issue> = iss.iter().filter(|i| i.severity != "info" || show_info).cloned().collect();
                match issues {
                    "any" if shown.is_empty() => continue,
                    "error" if !shown.iter().any(|i| i.severity == "error") => continue,
                    "warning" if !shown.iter().any(|i| i.severity == "warning") => continue,
                    "overflow" if !iss.iter().any(|i| i.code == "textbox-overflow") => continue,
                    _ => {}
                }
                if let Some(m) = &matcher {
                    let speaker = o.store.glossary.get(&Self::speaker_code(&e.who)).map_or("", |g| g.n.as_str());
                    if !(m(&e.target) || m(e.source.as_deref().unwrap_or("")) || m(&e.who) || m(&rel) || m(speaker)) {
                        continue;
                    }
                }
                for (k, v) in &member {
                    if *v {
                        counts.iter_mut().find(|(c, _)| c == k).unwrap().1 += 1;
                    }
                }
                if let Some((_, in_status)) = member.iter().find(|(k, _)| *k == status) {
                    if !in_status {
                        continue;
                    }
                }
                rows.push((rel.clone(), e.clone(), st, shown, pf));
            }
        }
        match sort {
            "kind" => rows.sort_by_key(|r| (kind_rank(r.1.category), r.0.to_lowercase(), r.1.line)),
            "issues" => rows.sort_by_key(|r| (-r.3.iter().map(|i| if i.severity == "error" { 3 } else { 1 }).sum::<i64>(), r.0.to_lowercase(), r.1.line)),
            "file" => rows.sort_by_key(|r| (r.0.to_lowercase(), r.1.line)),
            _ => {}
        }
        let total = rows.len();
        let mut offset = offset.max(0) as usize;
        if around != 0 {
            if let Some(idx) = rows.iter().position(|r| r.1.line as i64 == around && (file.is_empty() || r.0 == file)) {
                offset = idx.saturating_sub(30);
            }
        }
        let items: Vec<Value> = rows
            .iter()
            .skip(offset)
            .take(limit)
            .map(|(rel, e, st, shown, pf)| {
                json!({
                    "file": rel, "line": e.line, "src_line": e.src_line, "block": e.block, "who": e.who, "source": e.source, "target": e.target, "status": st, "kind": e.category,
                    "proof": pf, "unit": e.unit, "ref": e.reference, "issues": shown.iter().map(|i| json!({"severity": i.severity, "code": i.code})).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(
            json!({"total": total, "offset": offset, "counts": counts.into_iter().collect::<Vec<_>>().iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<String, Value>>(), "items": items}),
        )
    }

    pub(crate) fn find_entry(&self, rel: &str, line: i64) -> Option<(usize, &Entry)> {
        let o = self.open.as_ref()?;
        if !o.project.files.contains_key(rel) {
            return None;
        }
        o.project.entries(rel).iter().enumerate().find(|(_, e)| e.line as i64 == line)
    }

    pub fn entry_detail(&mut self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let (idx, e) = self.find_entry(rel, line).ok_or_else(line_gone)?;
        let e = e.clone();
        let on = self.cfg_bool("validation.enabled", true);
        let iss: Vec<validator::Issue> =
            if on { self.issues(rel, false)?.into_iter().filter(|(ln, _, _)| *ln as i64 == line).map(|r| r.1).collect() } else { Vec::new() };
        let ign: Vec<validator::Issue> =
            if on { self.issues(rel, true)?.into_iter().filter(|(ln, _, _)| *ln as i64 == line).map(|r| r.1).collect() } else { Vec::new() };
        let terms = self.terms();
        let o = self.o()?;
        let ents = o.project.entries(rel);
        let brief = |x: &Entry| json!({"line": x.line, "who": x.who, "source": x.source, "target": x.target, "status": self.entry_row(rel, x), "proof": self.proof_if_translated(rel, x)});
        let mark = o.store.proof.get(rel).and_then(|t| t.get(&e.key()));
        let mut issues: Vec<Value> = iss
            .iter()
            .map(|i| {
                let mut v = i.to_json();
                v["ignored"] = json!(false);
                v
            })
            .collect();
        for i in &ign {
            let mut v = i.to_json();
            v["ignored"] = json!(true);
            v["ignore_note"] = self.ignored_mark(rel, Some(&e), i).and_then(|m| m.get("n").cloned()).unwrap_or(json!(""));
            issues.push(v);
        }
        let used_terms: Vec<Value> = terms
            .iter()
            .filter(|(en, _)| e.source.as_deref().is_some_and(|s| validator::term_in_text(s, en)))
            .map(|(en, cz)| json!({"en": en, "cz": cz, "used": validator::term_used(&e.target, cz, &o.pack)}))
            .collect();
        Ok(json!({
            "file": rel, "path": o.project.root.join(rel).to_string_lossy(), "line": e.line, "src_line": e.src_line, "block": e.block, "who": e.who, "unit": e.unit, "ref": e.reference,
            "source": e.source, "target": e.target, "src_raw": e.src_raw, "cur_raw": e.cur_raw, "status": self.entry_row(rel, &e), "kind": e.category,
            "proof": self.proof_if_translated(rel, &e),
            "proof_note": mark.and_then(|m| m.get("n").cloned()).unwrap_or(json!("")), "proof_date": mark.and_then(|m| m.get("d").cloned()).unwrap_or(json!("")),
            "key": [e.block, source_or_target(&e)],
            "terms": used_terms, "issues": issues,
            "before": ents[idx.saturating_sub(2)..idx].iter().map(brief).collect::<Vec<_>>(),
            "after": ents[(idx + 1).min(ents.len())..(idx + 3).min(ents.len())].iter().map(brief).collect::<Vec<_>>(),
        }))
    }

    // ------------------------------------------------------------ mutations
    /// Save the counts of a file (the manual correction columns).
    pub fn save_file(&mut self, name: &str, c: &str, t: &str) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(name) {
            return Err(unknown_file());
        }
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let row = o.project.row(&o.store, name).ok_or_else(unknown_file)?;
        let (total, auto) = (row.total, row.auto);
        let c_n = parse_value(c, total);
        let mut t_n = parse_value(t, total);
        if t_n.is_some_and(|v| v == auto) {
            t_n = None;
        }
        // the number shown = hand-typed baseline + lines marked one by one; keep the marks, store the rest
        let c_n = match c_n {
            Some(v) if v != 0 => Some((v - o.project.proof_counts(&o.store, name).0).max(0)),
            other => other,
        };
        let label = Label::Msg(msg(
            "Editing the counts in {name}",
            &[("name", json!(std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))],
        ));
        let (c_text, t_text) = (c_n.filter(|v| *v != 0).map(|v| v.to_string()).unwrap_or_default(), t_n.map(|v| v.to_string()).unwrap_or_default());
        o.store.change(
            label,
            &[Kind::Corr],
            |st| {
                st.corr.insert(name.to_string(), Corr { c: c_text.clone(), t: t_text.clone() });
            },
            clock,
        )?;
        Ok(Value::Object(self.changed("save")?))
    }

    /// The original game script around the line this translation unit comes from (read-only).
    pub fn entry_context(&self, rel: &str, line: i64, radius: usize) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let (_, e) = self.find_entry(rel, line).ok_or_else(line_gone)?;
        let Some((script_rel, n)) = split_ref(&e.reference) else {
            return Ok(json!({"available": false, "reason": msg("The line has no recorded position in the game script.", &[]).wire()}));
        };
        let Some(base) = self.game_root() else {
            return Ok(
                json!({"available": false, "ref": e.reference, "reason": msg("The game folder (game) above the translation was not found.", &[]).wire()}),
            );
        };
        let path = resolve(&base.join(&script_rel));
        let suffix = path.extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
        if !below(&path, &resolve(&base)) || (suffix != ".rpy" && suffix != ".rpym") {
            return Ok(json!({"available": false, "ref": e.reference, "reason": msg("Invalid script path.", &[]).wire()}));
        }
        let lines = match std::fs::metadata(&path) {
            Ok(m) if m.len() > 8 * 1024 * 1024 => {
                return Ok(json!({"available": false, "ref": e.reference, "reason": msg("The script is too large.", &[]).wire()}))
            }
            Ok(_) => script_lines(&path),
            Err(e) => Err(e),
        };
        let Ok(lines) = lines else {
            return Ok(json!({"available": false, "ref": e.reference, "reason": msg(
                "The script {name} was not found (the original .rpy files of the game are not at hand, maybe only .rpyc).", &[("name", json!(script_rel))]).wire()}));
        };
        let (a, b) = (n.saturating_sub(radius).max(1), (n + radius).min(lines.len()));
        let shown: Vec<Value> = (a..=b).map(|i| json!({"n": i, "text": lines[i - 1], "hit": i == n})).collect();
        Ok(json!({"available": true, "ref": e.reference, "path": path.to_string_lossy(), "lines": shown}))
    }

    /// True when the translation needs more room than the game's dialogue box has (and the original did not).
    pub fn overflows(&self, e: &Entry) -> bool {
        let Some(src) = e.source.as_deref().filter(|s| !s.is_empty()) else { return false };
        if e.target.is_empty() || e.unit == "string" {
            return false;
        }
        let tb = self.textbox_cfg();
        validator::check_textbox(src, &e.target, Some(&tb)).iter().any(|i| i.code == "textbox-overflow")
    }

    /// The next line (after `line`, wrapping round the file) that is untranslated (`todo`), still to be checked (`review`), marked for a fix (`fix`) or does not fit the
    /// dialogue box (`over`). `{line: 0}` when there is none.
    pub fn entry_next(&self, rel: &str, line: i64, kind: &str) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        if !["todo", "review", "fix", "over"].contains(&kind) {
            return Err(app_err(msg("Unknown kind of line.", &[])));
        }
        let o = self.o()?;
        let ents = o.project.entries(rel);
        let matches = |e: &Entry| {
            let st = self.entry_row(rel, e);
            if kind == "todo" {
                return st == "todo";
            }
            if st == "todo" {
                return false;
            }
            if kind == "over" {
                return self.overflows(e);
            }
            let pf = o.project.proof_state(&o.store, rel, e);
            if kind == "review" {
                pf != "ok" && pf != "fix"
            } else {
                pf == "fix"
            }
        };
        let idx: isize = ents.iter().position(|e| e.line as i64 == line).map_or(-1, |i| i as isize);
        let order = ((idx + 1) as usize..ents.len()).chain(0..(idx + 1).max(0) as usize);
        for i in order {
            if matches(&ents[i]) {
                return Ok(json!({"line": ents[i].line, "wrapped": (i as isize) <= idx}));
            }
        }
        Ok(json!({"line": 0, "wrapped": false}))
    }

    /// The scenes of a file (runs of lines that follow the same `scene` statement of the game script) with their progress, and the one that holds `line`. Needs the game sources.
    pub fn scene_list(&mut self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let (gi, base) = (self.game_images(), self.game_root());
        let (Some(gi), Some(base)) = (gi, base) else {
            return Ok(
                json!({"available": false, "items": [], "cur": -1, "reason": msg("The game folder (game) above the translation was not found.", &[]).wire()}),
            );
        };
        let root = resolve(&base);
        let o = self.o()?;
        let mut items: Vec<Map<String, Value>> = Vec::new();
        let mut key: Option<(String, usize)> = None;
        let mut cur: i64 = -1;
        let mut starts: std::collections::HashMap<std::path::PathBuf, Vec<(usize, String)>> = std::collections::HashMap::new();
        for e in o.project.entries(rel) {
            let mut name: Option<String> = None;
            let k: Option<(String, usize)> = match split_ref(&e.reference) {
                Some((script_rel, n)) => {
                    let script = resolve(&base.join(&script_rel));
                    let suffix = script.extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
                    if below(&script, &root) && (suffix == ".rpy" || suffix == ".rpym") {
                        let st = starts
                            .entry(script.clone())
                            .or_insert_with(|| crate::gameimages::GameImages::scene_starts(&script_lines(&script).unwrap_or_default()));
                        let pos = st.partition_point(|s| s.0 <= n).checked_sub(1);
                        name = Some(pos.map_or(String::new(), |p| st[p].1.clone()));
                        Some((script_rel, pos.map_or(0, |p| st[p].0)))
                    } else {
                        key.clone()
                    }
                }
                None => key.clone(),
            };
            if items.is_empty() || k != key {
                let mut it = Map::new();
                it.insert("line".into(), json!(e.line));
                it.insert("count".into(), json!(0));
                it.insert("done".into(), json!(0));
                it.insert("proofed".into(), json!(0));
                it.insert("name".into(), json!(name.clone().unwrap_or_default()));
                it.insert("script".into(), json!(k.as_ref().map_or(String::new(), |k| format!("{}:{}", k.0, k.1))));
                items.push(it);
                key = k;
            }
            let it = items.last_mut().unwrap();
            let bump = |it: &mut Map<String, Value>, f: &str| it[f] = json!(it[f].as_i64().unwrap_or(0) + 1);
            bump(it, "count");
            if self.entry_row(rel, e) != "todo" {
                bump(it, "done");
                if o.project.proof_state(&o.store, rel, e) == "ok" {
                    bump(it, "proofed");
                }
            }
            if e.line as i64 <= line {
                cur = items.len() as i64 - 1;
            }
        }
        let _ = gi;
        Ok(json!({"available": true, "items": items, "cur": cur}))
    }

    /// The lines around one line (original and translation), i.e. the scene it belongs to.
    pub fn entry_scene(&self, rel: &str, line: i64, radius: i64) -> Res<Value> {
        self.need_project()?;
        if !self.o()?.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let (idx, _) = self.find_entry(rel, line).ok_or_else(line_gone)?;
        let ents = self.o()?.project.entries(rel);
        let r = radius.clamp(1, 30) as usize;
        let (a, b) = (idx.saturating_sub(r), (idx + r + 1).min(ents.len()));
        let items: Vec<Value> = ents[a..b]
            .iter()
            .map(|e| {
                json!({"line": e.line, "who": e.who, "source": e.source, "target": e.target, "status": self.entry_row(rel, e), "current": e.line as i64 == line, "unit": e.unit,
                    "over": self.overflows(e), "proof": self.proof_if_translated(rel, e)})
            })
            .collect();
        Ok(json!({"file": rel, "items": items}))
    }

    /// Validator result for a text the translator is typing (nothing is written).
    pub fn entry_check(&mut self, rel: &str, line: i64, text: &str) -> Res<Value> {
        self.need_project()?;
        let (_, ent) = self.find_entry(rel, line).ok_or_else(line_gone)?;
        let mut probe = ent.clone();
        let literal = match fixes::check_literal(text) {
            Ok(()) => String::new(),
            Err(e) => e.0.text(),
        };
        probe.target = text.to_string();
        probe.category = "";
        let (length, textbox, terms) = (self.length_cfg(), self.textbox_cfg(), self.terms());
        let spell = self.spell();
        let show_info = self.cfg_bool("validation.show_info", false);
        let o = self.o()?;
        let checks =
            Checks { length_cfg: Some(&length), terms: &terms, spell: spell.as_deref().map(|s| s as &dyn validator::SpellCheck), textbox: Some(&textbox) };
        let issues: Vec<Value> =
            validator::check_entry(&probe, &o.pack, &checks).iter().filter(|i| i.severity != "info" || show_info).map(|i| i.to_json()).collect();
        Ok(json!({"literal_error": literal, "issues": issues}))
    }

    /// Write a new translation of one line into the .rpy file (backup, atomic, verified, undoable).
    pub fn entry_save(&mut self, rel: &str, line: i64, text: &str) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let res = fixes::edit_translation(&o.project, &o.pack, &o.local_dir, rel, line.max(0) as usize, text, clock);
        let _ = o.project.refresh(true, clock); // whatever happened, the files are read again
        o.val_cache.clear();
        let res = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        let details: Vec<Value> = res.get("details").and_then(Value::as_array).cloned().unwrap_or_default();
        self.log_text(&details, "edit");
        let mut out = self.changed("edit")?;
        if let Value::Object(r) = res {
            out.extend(r.into_iter().filter(|(k, _)| k != "details"));
        }
        Ok(Value::Object(out))
    }

    /// Record changed texts (from `apply_edits`) in the line history. Never lets a logging problem break an edit.
    pub fn log_text(&mut self, details: &[Value], via: &str) {
        let clock = &*self.clock;
        let Some(o) = self.open.as_mut() else { return };
        let items: Vec<(String, (String, String), String, String)> = details
            .iter()
            .map(|d| {
                let key = d["key"].as_array().cloned().unwrap_or_default();
                let text = |v: &Value| v.as_str().unwrap_or("").to_string();
                (text(&d["file"]), (text(key.first().unwrap_or(&Value::Null)), text(key.get(1).unwrap_or(&Value::Null))), text(&d["old"]), text(&d["new"]))
            })
            .collect();
        o.texthist.record(&items, via, clock);
        self.team_activity(details, via); // in a team: what this person changed (for the statistics of the lead)
    }

    /// All texts this line has had, newest first.
    pub fn entry_history(&self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let (_, ent) = self.find_entry(rel, line).ok_or_else(line_gone)?;
        let key = ent.key();
        let versions = self.o()?.texthist.versions(rel, (&key.0, &key.1), &ent.target);
        let count = versions.len().saturating_sub(1);
        Ok(json!({"versions": versions, "count": count}))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_files(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(json!({"files": app.files_payload()?}))
}

pub fn op_entries(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entries(
        a.s("file"),
        a.s("status"),
        a.s("kind"),
        a.s("q"),
        a.s("issues"),
        a.s("sort"),
        a.i("offset"),
        a.i("limit"),
        a.i("around"),
        a.s("unit"),
        a.f("word"),
    )?)
}

pub fn op_entry(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_detail(a.s("file"), a.i("line"))?)
}

pub fn op_entry_next(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_next(a.s("file"), a.i("line"), a.s("kind"))?)
}

pub fn op_scenes(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.scene_list(a.s("file"), a.i("line"))?)
}

pub fn op_entry_scene(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_scene(a.s("file"), a.i("line"), a.i("radius"))?)
}

pub fn op_entry_history(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_history(a.s("file"), a.i("line"))?)
}

pub fn op_entry_context(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_context(a.s("file"), a.i("line"), 6)?)
}

pub fn op_entry_check(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_check(a.s("file"), a.i("line"), a.s("text"))?)
}

pub fn op_entry_save(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.entry_save(a.s("file"), a.i("line"), a.s("text"))?)
}

pub fn op_file_save(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.save_file(a.s("name"), a.s("c"), a.s("t"))?)
}
