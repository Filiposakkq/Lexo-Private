//! Port of `pro/lexo_pro/categorize.py`: suggesting which group (episode, chapter ...) a file belongs to, from its name, its folder and the assignments of its neighbours.
//!
//! Works on plain data: the request carries the names, the current and known assignments and the layout of the project (`Layout::to_json()`), so it needs nothing of the open project. It only
//! proposes; the assignments are written by Core when the translator accepts them.
use lexo_core::layout::{self, Layout};
use lexo_core::messages::{msg, py_str};
use lexo_core::store::{assign_label, valid_assign};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// File stems that belong to the whole game, not to one episode.
const SHARED_STEMS: [&str; 6] = ["header", "options", "screens", "gui", "common", "title_screens"];

fn season_re() -> &'static fancy_regex::Regex {
    static R: OnceLock<fancy_regex::Regex> = OnceLock::new();
    R.get_or_init(|| fancy_regex::Regex::new(r"(?<![a-z0-9])season[ _-]?0*(\d)(?!\d)").unwrap())
}

fn update_re_fancy() -> &'static fancy_regex::Regex {
    static R: OnceLock<fancy_regex::Regex> = OnceLock::new();
    R.get_or_init(|| fancy_regex::Regex::new(r"(?<![a-z0-9])update[ _-]?0*(\d{1,2})(?!\d)").unwrap())
}

fn posix(rel: &str) -> String {
    rel.replace('\\', "/").to_lowercase()
}

fn dirname(p: &str) -> &str {
    p.rsplit_once('/').map_or("", |x| x.0)
}

fn numbers(re: &fancy_regex::Regex, text: &str) -> Vec<i64> {
    let set: BTreeSet<i64> = re.captures_iter(text).filter_map(|c| c.ok()).filter_map(|c| c.get(1).and_then(|m| m.as_str().parse().ok())).collect();
    set.into_iter().collect()
}

/// (episodes, updates, seasons) named in a path.
fn signals(rel: &str, lay: &Layout) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let p = posix(rel);
    let mut found: BTreeSet<i64> = BTreeSet::new();
    let word = lay.labels.get("group").map_or("Episode", String::as_str);
    found.extend(numbers(layout::group_regex(word), &p));
    if let Some(n) = layout::series_number(&p, &lay.series) {
        if n != 0 {
            found.insert(n);
        }
    }
    if let Some(n) = layout::folder_number(&p, &lay.folders) {
        found.insert(n as i64);
    }
    (found.into_iter().collect(), numbers(update_re_fancy(), &p), numbers(season_re(), &p))
}

fn join_list(nums: &[i64]) -> String {
    nums.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
}

/// A `join([...], sep)` of messages and texts, as it travels.
fn joined(parts: Vec<Value>, sep: &str) -> Value {
    json!({"$j": parts, "sep": sep, "end": ""})
}

/// Propose an assignment for every file in `names`. `current` = what the translator has set (manual values are protected); `known` = the whole project's assignments, used to learn
/// folder conventions. One object per file: name, current, suggested (`""` = none), confidence (high|medium|low|none), state (new | same | conflict | ambiguous | unknown | keep), reason,
/// candidates, overwrite.
pub fn suggest(names: &[String], current: &Map<String, Value>, known: &Map<String, Value>, lay: &Layout) -> Vec<Value> {
    // folder -> {assignment: count}, in the order of first sight
    let mut by_dir: Vec<(String, Vec<(String, usize)>)> = Vec::new();
    for (f, v) in known {
        let v = py_str(v);
        if v.is_empty() {
            continue;
        }
        let dir = dirname(&posix(f)).to_string();
        let slot = match by_dir.iter().position(|(d, _)| *d == dir) {
            Some(i) => i,
            None => {
                by_dir.push((dir, Vec::new()));
                by_dir.len() - 1
            }
        };
        match by_dir[slot].1.iter_mut().find(|(k, _)| *k == v) {
            Some(e) => e.1 += 1,
            None => by_dir[slot].1.push((v, 1)),
        }
    }
    let active = lay.active_groups();
    let mut out = Vec::new();
    for name in names {
        let cur = current.get(name).map(py_str).unwrap_or_default();
        let (eps, updates, seasons) = signals(name, lay);
        let valid: Vec<i64> = eps.iter().copied().filter(|e| active.contains(e)).collect();
        let invalid: Vec<i64> = eps.iter().copied().filter(|e| !active.contains(e)).collect();
        let season_dirs: Vec<i64> = seasons.iter().copied().filter(|s| !lay.groups_of(*s).is_empty()).collect();
        let allowed: BTreeSet<i64> = season_dirs.iter().flat_map(|s| lay.groups_of(*s)).collect();
        let (mut sug, mut conf, mut cands) = (String::new(), "none", Vec::<String>::new());
        let mut reason: Value = json!("");
        let mut season_conflict = false;
        let mut note = json!("");
        if !invalid.is_empty() {
            note = msg(
                " (the episode number(s) {numbers} in the path are outside the active range 1–{max})",
                &[("numbers", json!(join_list(&invalid))), ("max", json!(active.iter().copied().max().unwrap_or(0)))],
            )
            .wire();
        }
        if valid.len() == 1 {
            let ep = valid[0];
            sug = format!("ep:{ep}");
            conf = "high";
            reason = msg("The name or folder contains “ep{ep}”.", &[("ep", json!(ep))]).wire();
            if !allowed.is_empty() && !allowed.contains(&ep) {
                conf = "low";
                season_conflict = true;
                reason = msg(
                    "The path contains episode {ep} (season {season}), but it lies in the folder of season {folders}.",
                    &[("ep", json!(ep)), ("season", json!(lay.section_of(ep))), ("folders", json!(join_list(&season_dirs)))],
                )
                .wire();
            }
        } else if valid.len() > 1 {
            cands = valid.iter().map(|e| format!("ep:{e}")).collect();
            let in_season: Vec<i64> = valid.iter().copied().filter(|e| allowed.is_empty() || allowed.contains(e)).collect();
            if in_season.len() == 1 && !allowed.is_empty() {
                sug = format!("ep:{}", in_season[0]);
                conf = "medium";
                reason = msg(
                    "There are several episodes in the path ({eps}); the season folder selects episode {ep}.",
                    &[("eps", json!(join_list(&valid))), ("ep", json!(in_season[0]))],
                )
                .wire();
            } else {
                reason = msg("Ambiguous: the path contains several episodes ({eps}).", &[("eps", json!(join_list(&valid)))]).wire();
            }
        } else if updates.len() == 1 && active.contains(&updates[0]) {
            let ep = updates[0];
            sug = format!("ep:{ep}");
            conf = "medium";
            reason = msg("The file “update{ep}” corresponds to episode {ep} (the convention of this project).", &[("ep", json!(ep))]).wire();
            if !allowed.is_empty() && !allowed.contains(&ep) {
                conf = "low";
                season_conflict = true;
                reason = joined(vec![reason, msg("The season folder does not fit this.", &[]).wire()], " ");
            }
        } else {
            let p = posix(name);
            let d = dirname(&p);
            let base = p.rsplit('/').next().unwrap_or("");
            let stem = base.rsplit_once('.').map_or(base, |x| x.0);
            let sib = by_dir.iter().find(|(k, _)| k == d).map(|(_, v)| v);
            if let Some(only) = sib.filter(|s| s.len() == 1 && !s[0].0.is_empty()) {
                sug = only[0].0.clone();
                conf = "medium";
                reason = msg("The other files in the same folder are assigned as “{label}”.", &[("label", assign_label(&sug, lay).wire())]).wire();
            } else if SHARED_STEMS.contains(&stem) {
                sug = "gen".to_string();
                conf = "medium";
                reason = msg("The name matches the files of the whole game (header, options, screens).", &[]).wire();
            } else if let Some(many) = sib.filter(|s| s.len() > 1) {
                let mut sorted: Vec<&(String, usize)> = many.iter().collect();
                sorted.sort_by_key(|a| std::cmp::Reverse(a.1)); // most common first; equal counts keep the order of first sight
                cands = sorted.iter().map(|(k, _)| k.clone()).collect();
                reason = msg("The files in the same folder are assigned to different episodes.", &[]).wire();
            } else if !allowed.is_empty() {
                reason = msg(
                    "The folder belongs to season {seasons} (episodes {a}–{b}), the exact episode cannot be told from the path.",
                    &[("seasons", json!(join_list(&season_dirs))), ("a", json!(allowed.iter().min())), ("b", json!(allowed.iter().max()))],
                )
                .wire();
            } else {
                reason = joined(vec![msg("Cannot be derived from the name or the folder.", &[]).wire(), note.clone()], "");
            }
        }
        if !sug.is_empty() && !valid_assign(&sug, lay) {
            sug = String::new();
            conf = "none";
        }
        let state = if sug.is_empty() && cands.is_empty() {
            if cur.is_empty() {
                "unknown"
            } else {
                "keep"
            }
        } else if sug.is_empty() {
            if cur.is_empty() {
                "ambiguous"
            } else {
                "keep"
            }
        } else if season_conflict {
            "conflict"
        } else if !cur.is_empty() && cur == sug {
            "same"
        } else if !cur.is_empty() {
            reason = joined(vec![reason, msg("It is set to “{label}” by hand.", &[("label", assign_label(&cur, lay).wire())]).wire()], " ");
            "conflict"
        } else {
            "new"
        };
        out.push(json!({"name": name, "current": cur, "suggested": sug, "confidence": conf, "state": state, "reason": reason, "candidates": cands,
            "overwrite": !cur.is_empty() && !sug.is_empty() && cur != sug}));
    }
    out
}
