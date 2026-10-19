//! Team work (the Pro feature `team.manage`): what the lead writes into the team file is checked and cleaned here. Core gathers who is in the team (`members`), the files of the project and
//! what the lead wants (`roles`, `assign`); this returns what may be written and how many entries it dropped. Rights are not the point (a shared folder is a team of trust), a clean file is:
//! roles are `translator` or `proofreader` (the lead has no role of their own), only members get files, only files of the project are assigned, and the lists have limits.
use lexo_core::messages::py_str;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const ROLES: [&str; 2] = ["translator", "proofreader"];
const MAX_PER_FILE: usize = 5;
const MAX_MEMBERS: usize = 100;

/// `{roles: {author: role}, assign: {file: [author]}, dropped}`.
pub fn normalize(p: &Value) -> Value {
    let members: BTreeSet<String> = p["members"].as_array().into_iter().flatten().map(py_str).take(MAX_MEMBERS).collect();
    let lead = py_str(&p["lead"]);
    let files: BTreeSet<String> = p["files"].as_array().into_iter().flatten().map(py_str).collect();
    let mut dropped = 0usize;
    let mut roles = Map::new();
    for (author, role) in p["roles"].as_object().into_iter().flatten() {
        let role = py_str(role);
        if members.contains(author) && *author != lead && ROLES.contains(&role.as_str()) {
            roles.insert(author.clone(), json!(role));
        } else {
            dropped += 1;
        }
    }
    let mut assign = Map::new();
    for (file, who) in p["assign"].as_object().into_iter().flatten() {
        if !files.contains(file) {
            dropped += 1;
            continue;
        }
        let mut list: Vec<String> = Vec::new();
        for a in who.as_array().into_iter().flatten().map(py_str) {
            if members.contains(&a) && !list.contains(&a) && list.len() < MAX_PER_FILE {
                list.push(a);
            } else {
                dropped += 1;
            }
        }
        if !list.is_empty() {
            assign.insert(file.clone(), json!(list));
        }
    }
    json!({"roles": roles, "assign": assign, "dropped": dropped})
}

fn pct(part: i64, whole: i64) -> f64 {
    if whole <= 0 {
        0.0
    } else {
        (part as f64 * 1000.0 / whole as f64).round() / 10.0
    }
}

/// Statistics per person for the lead. Core gathers the numbers (`counts`: what each person edited, approved, marked in the chosen period, `last`: the time of their last activity, `rows`: how far each
/// file is translated and proofread, `assign`, `members`); this joins them: for every member the progress of their files and what they did, and the progress of the files nobody has.
/// `{members: [{author, name, role, files, lines, done_pct, proof_pct, edited, approved, marked, proofread_updates, last}], unassigned: {files, lines, done_pct, proof_pct}}`.
pub fn stats(p: &Value) -> Value {
    let rows = p["rows"].as_object().cloned().unwrap_or_default();
    let assign = p["assign"].as_object().cloned().unwrap_or_default();
    let num = |v: &Value, k: &str| v[k].as_i64().unwrap_or(0);
    let mut members = Vec::new();
    for m in p["members"].as_array().into_iter().flatten() {
        let author = py_str(&m["author"]);
        let (mut files, mut lines, mut done, mut corr) = (0i64, 0i64, 0i64, 0i64);
        for (file, who) in &assign {
            if who.as_array().is_some_and(|a| a.iter().any(|x| py_str(x) == author)) {
                if let Some(r) = rows.get(file) {
                    files += 1;
                    lines += num(r, "total");
                    done += num(r, "done");
                    corr += num(r, "corr");
                }
            }
        }
        let c = &p["counts"][author.as_str()];
        members.push(json!({
            "author": author, "name": m["name"], "role": m["role"], "files": files, "lines": lines, "done_pct": pct(done, lines), "proof_pct": pct(corr, lines),
            "edited": num(c, "edited"), "approved": num(c, "approved"), "marked": num(c, "marked"), "proofread_updates": num(c, "proofread_updates"),
            "last": p["last"][author.as_str()].clone(),
        }));
    }
    let owned: BTreeSet<&String> = assign.iter().filter(|(_, v)| v.as_array().is_some_and(|a| !a.is_empty())).map(|(k, _)| k).collect();
    let (mut files, mut lines, mut done, mut corr) = (0i64, 0i64, 0i64, 0i64);
    for (file, r) in &rows {
        if !owned.contains(file) {
            files += 1;
            lines += num(r, "total");
            done += num(r, "done");
            corr += num(r, "corr");
        }
    }
    json!({"members": members, "unassigned": {"files": files, "lines": lines, "done_pct": pct(done, lines), "proof_pct": pct(corr, lines)}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_join_the_progress_of_a_persons_files_with_what_they_did() {
        let out = stats(&json!({
            "members": [{"author": "a-1", "name": "Ann", "role": "translator"}, {"author": "b-2", "name": "Bob", "role": "proofreader"}],
            "assign": {"x.rpy": ["a-1"], "y.rpy": ["a-1", "b-2"]},
            "rows": {"x.rpy": {"total": 100, "done": 50, "corr": 10}, "y.rpy": {"total": 100, "done": 100, "corr": 100}, "z.rpy": {"total": 40, "done": 4, "corr": 0}},
            "counts": {"a-1": {"edited": 7, "approved": 2}}, "last": {"a-1": "2026-10-19T10:00:00.000000+00:00"}
        }));
        let ann = &out["members"][0];
        assert_eq!(
            (ann["files"].clone(), ann["lines"].clone(), ann["done_pct"].clone(), ann["proof_pct"].clone()),
            (json!(2), json!(200), json!(75.0), json!(55.0))
        );
        assert_eq!((ann["edited"].clone(), ann["approved"].clone(), ann["marked"].clone()), (json!(7), json!(2), json!(0)));
        let bob = &out["members"][1];
        assert_eq!((bob["files"].clone(), bob["done_pct"].clone(), bob["last"].clone()), (json!(1), json!(100.0), json!(null)));
        assert_eq!(out["unassigned"], json!({"files": 1, "lines": 40, "done_pct": 10.0, "proof_pct": 0.0}));
    }

    #[test]
    fn only_members_get_roles_and_files_and_the_rest_is_dropped() {
        let out = normalize(&json!({
            "members": ["lead-1", "ann-2", "bob-3"], "lead": "lead-1", "files": ["a.rpy", "b.rpy"],
            "roles": {"ann-2": "proofreader", "bob-3": "boss", "ghost-9": "translator", "lead-1": "translator"},
            "assign": {"a.rpy": ["ann-2", "ann-2", "ghost-9"], "gone.rpy": ["bob-3"], "b.rpy": []}
        }));
        assert_eq!(out["roles"], json!({"ann-2": "proofreader"}));
        assert_eq!(out["assign"], json!({"a.rpy": ["ann-2"]}));
        assert_eq!(out["dropped"], 6); // boss, ghost role, the lead's role, the second ann, ghost file owner, the unknown file
    }
}
