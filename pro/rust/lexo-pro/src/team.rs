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

#[cfg(test)]
mod tests {
    use super::*;

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
