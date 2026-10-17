//! Port of `core/lexo/app_menu.py`: the menu a choice belongs to: question, the other choices, and what follows each of them (read from the game script).
use crate::api::{ok, Answer, Args};
use crate::app::{line_gone, unknown_file, App};
use crate::app_base::Res;
use crate::app_game::{script_lines, split_ref};
use crate::fsutil::resolve;
use crate::gametools::below;
use crate::menus;
use crate::messages::msg;
use crate::rpy::Entry;
use crate::util::{norm_ws, py_splitlines, py_strip};
use indexmap::IndexMap;
use serde_json::{json, Value};

/// `textwrap.dedent` of the lines of a menu body, split into lines again.
fn dedent_lines(lines: &[String]) -> Vec<String> {
    let text = lines.join("\n");
    // the margin is the longest common leading white space of the lines that are not blank
    let indent_of = |l: &str| l.chars().take_while(|c| *c == ' ' || *c == '\t').collect::<String>();
    let mut margin: Option<String> = None;
    for l in text.split('\n') {
        if py_strip(l).is_empty() {
            continue;
        }
        let ind = indent_of(l);
        margin = Some(match margin {
            None => ind,
            Some(m) => m.chars().zip(ind.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a).collect(),
        });
    }
    let margin = margin.unwrap_or_default();
    let dedented: Vec<String> =
        text.split('\n').map(|l| if py_strip(l).is_empty() { String::new() } else { l.strip_prefix(margin.as_str()).unwrap_or(l).to_string() }).collect();
    py_splitlines(&dedented.join("\n"))
}

impl App {
    /// Menu around the choice at `line` of the translation file `rel`. `{available: false, reason}` when the line is no choice of a menu we can read.
    pub fn entry_menu(&self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        if !o.project.files.contains_key(rel) {
            return Err(unknown_file());
        }
        let ent = o.project.entries(rel).iter().find(|e| e.line as i64 == line).ok_or_else(line_gone)?;
        let base = self.game_root();
        let (Some((script_rel, here_line)), Some(base)) = (split_ref(&ent.reference), base) else {
            return Ok(json!({"available": false, "reason": msg("The line has no recorded position in the game script.", &[]).wire()}));
        };
        let path = resolve(&base.join(&script_rel));
        let suffix = path.extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
        if !below(&path, &resolve(&base)) || (suffix != ".rpy" && suffix != ".rpym") {
            return Ok(json!({"available": false, "reason": msg("Invalid script path.", &[]).wire()}));
        }
        let lines = match std::fs::metadata(&path) {
            Ok(m) if m.len() > 8 * 1024 * 1024 => return Ok(json!({"available": false, "reason": msg("The script is too large.", &[]).wire()})),
            Ok(_) => script_lines(&path),
            Err(e) => Err(e),
        };
        let Ok(lines) = lines else {
            return Ok(json!({"available": false, "reason": msg(
                "The script {name} was not found (the original .rpy files of the game are not at hand, maybe only .rpyc).", &[("name", json!(script_rel))]).wire()}));
        };
        let choice = (ent.unit == "string").then_some(ent.source.as_deref()).flatten();
        let Some(found) = menus::menu_around(&lines, here_line, choice) else {
            return Ok(json!({"available": false, "reason": msg("This line is not part of a menu in the game script.", &[]).wire()}));
        };
        let mut by_ref: IndexMap<String, (&String, &Entry)> = IndexMap::new();
        let mut by_text: IndexMap<String, (&String, &Entry)> = IndexMap::new();
        for (r, fi) in &o.project.files {
            for e in &fi.parsed.entries {
                if e.unit == "string" {
                    if let Some(s) = &e.source {
                        by_text.entry(norm_ws(s)).or_insert((r, e));
                    }
                } else if !e.reference.is_empty() {
                    by_ref.insert(e.reference.clone(), (r, e));
                }
            }
        }
        let row = |r: &str, e: &Entry| json!({"file": r, "line": e.line, "who": e.who, "source": e.source, "target": e.target, "status": self.entry_row(r, e)});
        let say = |n: usize| {
            let hit = by_ref.get(&format!("{script_rel}:{n}"));
            json!({"n": n, "entry": hit.map(|(r, e)| row(r, e)), "script": if hit.is_some() { String::new() } else { py_strip(&lines[n - 1]).to_string() },
                "here": n == here_line && ent.unit != "string"})
        };
        let mut options: Vec<Value> = Vec::new();
        for (k, opt) in found.options.iter().enumerate() {
            let hit = by_text.get(&norm_ws(&opt.text));
            let here = [format!("{script_rel}:{}", found.menu_line), format!("{script_rel}:{}", opt.n)];
            // the same text stands in an earlier menu: Ren'Py lists it there once
            let where_ = hit.map(|(_, e)| e.reference.clone()).filter(|r| !r.is_empty() && !here.contains(r)).unwrap_or_default();
            options.push(json!({
                "n": opt.n, "text": opt.text, "cond": opt.cond, "current": k as i64 == found.current, "where": where_, "entry": hit.map(|(r, e)| row(r, e)),
                "after": opt.after.iter().map(|n| say(*n)).collect::<Vec<_>>(),
                "body": if opt.after.is_empty() { dedent_lines(&opt.body) } else { Vec::new() },
            }));
        }
        Ok(json!({
            "available": true, "script": format!("{script_rel}:{}", found.menu_line), "question": found.question.iter().map(|n| say(*n)).collect::<Vec<_>>(),
            "before": found.before.iter().map(|n| say(*n)).collect::<Vec<_>>(), "options": options,
        }))
    }
}

pub fn op_entry_menu(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.entry_menu(a.s("file"), a.i("line"))?)
}
