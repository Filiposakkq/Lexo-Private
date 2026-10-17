//! Port of `core/lexo/bulk.py`: bulk operations: applying reviewed group assignments and global approval of identical lines.
//!
//! The suggestions of groups for files are a Pro feature (the Pro pack, `bulk.categorize`); applying them, and the approval of identical lines, are here. Both only
//! *propose* what they show; nothing changes until the caller applies the reviewed result.
use crate::clock::Clock;
use crate::fsutil::WriteFailed;
use crate::messages::{msg, Message};
use crate::project::Project;
use crate::rpy::{self, Entry};
use crate::store::{valid_assign, Kind, Label, Store};
use crate::util::norm_ws;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

/// One reviewed assignment: a manual value is only replaced when `overwrite` is true.
pub struct Assignment {
    pub name: String,
    pub value: String,
    pub overwrite: bool,
}

/// What `apply_assignments` did: the names applied and the ones skipped with the reason.
pub type AssignOutcome = (Vec<String>, Vec<(String, Message)>);

/// Apply reviewed assignments in one undoable step. Returns (applied names, skipped (name, reason)).
pub fn apply_assignments(store: &mut Store, items: &[Assignment], clock: &dyn Clock) -> Result<AssignOutcome, WriteFailed> {
    let layout = store.layout.clone();
    let (mut applied, mut skipped) = (Vec::new(), Vec::new());
    let label = Label::Msg(msg("Assignment of files ({n})", &[("n", json!(items.len()))]));
    store.change(
        label,
        &[Kind::Assign],
        |st| {
            for it in items {
                if !valid_assign(&it.value, &layout) {
                    skipped.push((it.name.clone(), msg("invalid assignment", &[])));
                    continue;
                }
                let cur = st.assign.get(&it.name).cloned().unwrap_or_default();
                if cur == it.value {
                    continue;
                }
                if !cur.is_empty() && !it.overwrite {
                    skipped.push((it.name.clone(), msg("has an assignment set by hand (the overwrite was not confirmed)", &[])));
                    continue;
                }
                if !it.value.is_empty() {
                    st.assign.insert(it.name.clone(), it.value.clone());
                } else {
                    st.assign.shift_remove(&it.name);
                }
                applied.push(it.name.clone());
            }
        },
        clock,
    )?;
    Ok((applied, skipped))
}

// ---------------------------------------------------------------- global approval

pub const SCOPES: [&str; 3] = ["occurrence", "file", "project"];

fn sorted_files(project: &Project) -> Vec<String> {
    let mut f: Vec<String> = project.files.keys().cloned().collect();
    f.sort_by_key(|r| r.to_lowercase());
    f
}

/// Find lines that are identical to the chosen pending line and could be approved together. A match needs the same normalised source *and* translation. Everything
/// else about the context is shown and decides the default checkbox: a different speaker is unchecked, empty translations can never be approved, lines that look like
/// untranslated English are unchecked, already approved lines are listed but not selectable.
pub fn approval_candidates(project: &Project, store: &Store, rel: &str, line: usize, scope: &str, same_speaker: bool) -> Result<Value, Message> {
    if !SCOPES.contains(&scope) {
        return Err(msg("unknown scope", &[]));
    }
    let Some(rf) = project.entries(rel).iter().find(|e| e.line == line).filter(|e| e.pending()) else {
        return Err(msg("The line is not among the untranslated ones.", &[]));
    };
    let (src, tgt) = (norm_ws(rf.source.as_deref().unwrap_or("")), norm_ws(&rf.target));
    let files: Vec<String> = if scope == "occurrence" || scope == "file" { vec![rel.to_string()] } else { sorted_files(project) };
    let empty = BTreeSet::new();
    let mut items: Vec<Value> = Vec::new();
    for f in &files {
        let done = store.approved.get(f).unwrap_or(&empty);
        for e in project.entries(f) {
            if !e.pending() || norm_ws(e.source.as_deref().unwrap_or("")) != src || norm_ws(&e.target) != tgt {
                continue;
            }
            if scope == "occurrence" && e.line != line {
                continue;
            }
            let already = done.contains(&e.key());
            let same_who = norm_ws(&e.who) == norm_ws(&rf.who);
            let mut reasons: Vec<Value> = Vec::new();
            let (mut selectable, mut default) = (true, true);
            if already {
                selectable = false;
                default = false;
                reasons.push(msg("already approved", &[]).wire());
            }
            if e.category == rpy::K_EMPTY {
                selectable = false;
                default = false;
                reasons.push(msg("an empty translation cannot be approved", &[]).wire());
            }
            if !same_who && same_speaker {
                default = false;
                let dash = |s: &str| if s.is_empty() { "—".to_string() } else { s.to_string() };
                reasons.push(msg("another speaker (“{a}” instead of “{b}”)", &[("a", json!(dash(&e.who))), ("b", json!(dash(&rf.who)))]).wire());
            }
            if e.category == rpy::K_ENGLISH && default {
                default = false;
                reasons.push(msg("looks like untranslated English", &[]).wire());
            }
            let is_ref = f == rel && e.line == rf.line;
            if is_ref {
                default = selectable;
            }
            items.push(json!({
                "file": f, "line": e.line, "block": e.block, "who": e.who, "text": e.target, "kind": e.category, "key": [e.block, norm_ws(e.source.as_deref().unwrap_or(""))],
                "approved": already, "selectable": selectable, "selected": default, "notes": reasons, "is_reference": is_ref,
            }));
        }
    }
    let selected = items.iter().filter(|i| i["selected"] == json!(true)).count();
    let nfiles = items.iter().map(|i| i["file"].as_str().unwrap_or("").to_string()).collect::<HashSet<_>>().len();
    Ok(json!({
        "reference": {"file": rel, "line": line, "text": rf.target, "who": rf.who}, "scope": scope,
        "counts": {"total": items.len(), "selected": selected, "files": nfiles}, "items": items,
    }))
}

/// A line chosen for approval: the file and the key (block, source).
pub struct Selection {
    pub file: String,
    pub key: (String, String),
}

/// Approve / un-approve in one undoable step. Each selection is re-checked against the current parse, so a file that changed since the preview cannot get a stale approval.
/// Returns (changed, skipped (file, reason)).
pub fn apply_approvals(
    project: &Project,
    store: &mut Store,
    selections: &[Selection],
    on: bool,
    label: Option<Label>,
    clock: &dyn Clock,
) -> Result<(usize, Vec<(String, Message)>), WriteFailed> {
    let mut skipped: Vec<(String, Message)> = Vec::new();
    let mut plan: Vec<(String, BTreeSet<(String, String)>)> = Vec::new();
    for sel in selections {
        let key = (sel.key.0.clone(), norm_ws(&sel.key.1));
        if !project.files.contains_key(&sel.file) {
            skipped.push((sel.file.clone(), msg("unknown file", &[])));
            continue;
        }
        if on {
            let Some(ent) = project.entries(&sel.file).iter().find(|e| e.pending() && e.key() == key) else {
                skipped.push((sel.file.clone(), msg("the line changed in the meantime – refresh the files", &[])));
                continue;
            };
            if ent.category == rpy::K_EMPTY {
                skipped.push((sel.file.clone(), msg("an empty translation cannot be approved", &[])));
                continue;
            }
        }
        match plan.iter_mut().find(|(f, _)| *f == sel.file) {
            Some((_, keys)) => {
                keys.insert(key);
            }
            None => plan.push((sel.file.clone(), BTreeSet::from([key]))),
        }
    }
    let n: usize = plan.iter().map(|(_, k)| k.len()).sum();
    let label = label.unwrap_or_else(|| {
        Label::Msg(if on { msg("Approving {n} lines", &[("n", json!(n))]) } else { msg("Removing the approval of {n} lines", &[("n", json!(n))]) })
    });
    let mut changed = 0usize;
    store.change(
        label,
        &[Kind::Approved],
        |st| {
            for (f, keys) in &plan {
                let cur: BTreeSet<(String, String)> = st.approved.get(f).cloned().unwrap_or_default();
                let new: BTreeSet<(String, String)> = if on { cur.union(keys).cloned().collect() } else { cur.difference(keys).cloned().collect() };
                changed += new.symmetric_difference(&cur).count();
                if new.is_empty() {
                    st.approved.shift_remove(f);
                } else {
                    st.approved.insert(f.clone(), new);
                }
            }
        },
        clock,
    )?;
    Ok((changed, skipped))
}

// ---------------------------------------------------------------- approval by rule

/// The rules and the text that says what each one does.
pub fn rules() -> Vec<(&'static str, Message)> {
    vec![
        ("symbols", msg("Symbols only (…, ?!, numbers) – nothing to translate", &[])),
        ("interjections", msg("Interjections (Hmm, Ah, Haha, Wow, Ugh…) – the same in the translation", &[])),
        ("names", msg("Only names from the Glossary (speakers, terms)", &[])),
    ]
}

const INTERJECTIONS: [&str; 42] = [
    "hm", "hmm", "hmmm", "mm", "mmm", "mhm", "ah", "aha", "ahh", "oh", "ooh", "uh", "uhm", "um", "umm", "ugh", "ha", "haha", "hahaha", "hehe", "heh", "hihi",
    "wow", "whoa", "yay", "ow", "ouch", "argh", "eh", "huh", "shh", "psst", "ok", "okay", "ahem", "pfft", "tsk", "tss", "boo", "bam", "pow", "zzz",
];

fn words_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[^\W\d_]+(?:'[^\W\d_]+)?").unwrap())
}

fn plain_markup(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{[^}]*\}|\[[^\]]*\]").unwrap()).replace_all(&text.replace("\\n", " "), " ").into_owned()
}

/// Lower-case words of every speaker name and glossary term (both the English and the Czech side). `glossary` = [(code, full name)].
pub fn name_words(glossary: &[(String, String)]) -> HashSet<String> {
    let mut out = HashSet::new();
    for (code, name) in glossary {
        for part in [code, name] {
            out.extend(words_re().find_iter(part).map(|m| m.as_str().to_lowercase()));
        }
    }
    out
}

/// Pending lines that a simple rule says need no translation. Nothing is changed. `rels` limits the search to those files (`None` / empty = the whole project).
/// An already approved line is listed but not selectable; an empty translation never qualifies.
pub fn rule_candidates(project: &Project, store: &Store, glossary: &[(String, String)], chosen: &[String], rels: Option<&[String]>) -> Value {
    let known: Vec<&str> = rules().iter().map(|r| r.0).collect();
    let chosen: Vec<&str> = chosen.iter().map(String::as_str).filter(|r| known.contains(r)).collect();
    let names = if chosen.contains(&"names") { name_words(glossary) } else { HashSet::new() };
    let mut files: Vec<String> = match rels {
        Some(r) if !r.is_empty() => r.iter().filter(|f| project.files.contains_key(*f)).cloned().collect(),
        _ => sorted_files(project),
    };
    let nfiles = files.len();
    files.sort_by_key(|r| r.to_lowercase());
    let empty = BTreeSet::new();
    let mut items: Vec<Value> = Vec::new();
    let mut per_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &files {
        let done = store.approved.get(f).unwrap_or(&empty);
        for e in project.entries(f) {
            if !e.pending() || e.category == rpy::K_EMPTY {
                continue;
            }
            let words: Vec<String> = words_re().find_iter(&plain_markup(&e.target)).map(|m| m.as_str().to_lowercase()).collect();
            let hit = rule_hit(e, &words, &chosen, &names);
            let Some(hit) = hit else { continue };
            let already = done.contains(&e.key());
            *per_rule.entry(hit).or_insert(0) += 1;
            items.push(json!({
                "file": f, "line": e.line, "block": e.block, "who": e.who, "text": e.target, "rule": hit, "key": [e.block, norm_ws(e.source.as_deref().unwrap_or(""))],
                "approved": already, "selectable": !already, "selected": !already, "notes": if already { vec![msg("already approved", &[]).wire()] } else { Vec::new() },
            }));
        }
    }
    let rules_map: Map<String, Value> = rules().into_iter().map(|(k, m)| (k.to_string(), m.wire())).collect();
    let selectable = items.iter().filter(|i| i["selectable"] == json!(true)).count();
    json!({
        "items": items, "rules": rules_map, "files": nfiles,
        "counts": {"total": items.len(), "selectable": selectable, "by_rule": per_rule.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<String, Value>>()},
    })
}

fn rule_hit(e: &Entry, words: &[String], chosen: &[&str], names: &HashSet<String>) -> Option<&'static str> {
    if chosen.contains(&"symbols") && e.category == rpy::K_SYMBOLS {
        Some("symbols")
    } else if !words.is_empty() && chosen.contains(&"interjections") && words.iter().all(|w| INTERJECTIONS.contains(&w.as_str())) {
        Some("interjections")
    } else if !words.is_empty() && chosen.contains(&"names") && words.len() <= 4 && words.iter().all(|w| names.contains(w)) {
        Some("names")
    } else {
        None
    }
}
