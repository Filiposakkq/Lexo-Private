//! Port of `reference/python/core/lexo/fixes.py`: the only place where the manager changes a translation `.rpy` file.
//!
//! Operations: editing the translated text of one line in the app (`edit_translation`), writing many edits as one undoable step (`apply_edits`), and writing planned fixes of
//! the speaker / attributes of dialogue lines (`apply`). What to fix is planned elsewhere (the safe fixes are a Pro feature); this module only writes.
//!
//! Safety: a plan is built first (nothing written); applying re-reads the file and refuses when it changed on disk since it was parsed; a backup of the file is always made;
//! the write is atomic; the file is parsed again afterwards and the result is verified.
use crate::clock::Clock;
use crate::fsutil::{atomic_write_bytes, make_backup, WriteFailed};
use crate::lang::Pack;
use crate::messages::{msg, Message};
use crate::project::Project;
use crate::rpy::{self, Entry};
use crate::util::{is_space, py_strip, sha1_hex};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
pub const LAST_FILE: &str = "last_fix.json";
pub const STACK_MAX: usize = 30;

#[derive(Debug, PartialEq)]
pub struct FixError(pub Message);

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

fn fail<T>(m: Message) -> Result<T, Box<FixError>> {
    Err(Box::new(FixError(m)))
}

type Res<T> = Result<T, Box<FixError>>;

/// The text split at the line ends, the ends kept: text, eol, text, eol ... (line N is element 2 * (N - 1)).
fn split_lines_keeping(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' {
            let mut eol = c.to_string();
            if c == '\r' && chars.peek() == Some(&'\n') {
                eol.push(chars.next().unwrap());
            }
            parts.push(std::mem::take(&mut cur));
            parts.push(eol);
        } else {
            cur.push(c);
        }
    }
    parts.push(cur);
    parts
}

fn write_failed(e: &WriteFailed) -> Box<FixError> {
    Box::new(FixError(msg(
        "Cannot write: {reason} ({name})",
        &[("reason", e.reason.wire()), ("name", json!(e.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))],
    )))
}

fn cannot_read(rel: &str, e: &std::io::Error) -> Box<FixError> {
    Box::new(FixError(msg("{rel}: the file cannot be read ({reason}).", &[("rel", json!(rel)), ("reason", json!(e.to_string()))])))
}

/// One planned fix of the speaker / attributes of a dialogue line.
pub struct Planned {
    pub file: String,
    pub line: usize,
    pub before: String,
    pub after: String,
    /// Why it is not written, or `None`.
    pub skip: Option<Message>,
}

#[derive(Debug, Default)]
pub struct Applied {
    pub fixed: usize,
    pub skipped: Vec<(String, usize, Message)>,
    pub backups: Vec<String>,
}

fn decode_strict(bytes: &[u8]) -> Option<String> {
    String::from_utf8(bytes.to_vec()).ok()
}

/// The history record of what was written (for the undo of it), pushed on the stack in `last_fix.json`.
fn record_json(rel: &str, backup: &Path, after: &[u8], lines: usize) -> Value {
    json!({"file": rel, "backup": backup.to_string_lossy(), "after": sha1_hex(after), "lines": lines})
}

fn now_text(clock: &dyn Clock) -> String {
    clock.now_local().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Write the planned fixes. The numbers say how many lines were written, which were skipped and where the backups are.
pub fn apply(project: &Project, pack: &Pack, data_dir: &Path, planned: &[Planned], clock: &dyn Clock) -> Res<Applied> {
    let mut out = Applied::default();
    let mut by_file: BTreeMap<String, Vec<&Planned>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in planned {
        if let Some(why) = &p.skip {
            out.skipped.push((p.file.clone(), p.line, why.clone()));
        } else {
            if !by_file.contains_key(&p.file) {
                order.push(p.file.clone());
            }
            by_file.entry(p.file.clone()).or_default().push(p);
        }
    }
    let mut record: Vec<Value> = Vec::new();
    for rel in order {
        let items = &by_file[&rel];
        let full = project.path_of(&rel).map_err(|m| Box::new(FixError(m)))?;
        let fi = project.files.get(&rel);
        let data = std::fs::read(&full).map_err(|e| cannot_read(&rel, &e))?;
        let Some(fi) = fi.filter(|fi| rpy::parse_bytes(&data, pack).digest == fi.parsed.digest) else {
            return fail(msg("{rel}: the file changed on the disk since it was loaded. Refresh the files and try again.", &[("rel", json!(rel))]));
        };
        let had_bom = data.starts_with(&BOM);
        let Some(text) = decode_strict(if had_bom { &data[BOM.len()..] } else { &data }) else {
            return fail(msg("{rel}: the file is not valid UTF-8, I rather do not edit it.", &[("rel", json!(rel))]));
        };
        let mut parts = split_lines_keeping(&text);
        for p in items {
            let idx = 2 * (p.line.max(1) - 1);
            if idx >= parts.len() || py_strip(&parts[idx]) != p.before {
                return fail(msg(
                    "{rel}:{line}: the line no longer matches the loaded state. Refresh the files.",
                    &[("rel", json!(rel)), ("line", json!(p.line))],
                ));
            }
            let cur = parts[idx].clone();
            let lead_len = cur.len() - cur.trim_start_matches(is_space).len();
            let tail_start = cur.trim_end_matches(is_space).len();
            parts[idx] = format!("{}{}{}", &cur[..lead_len], p.after.trim_start_matches(is_space), &cur[tail_start..]);
        }
        let mut new_bytes = parts.concat().into_bytes();
        if had_bom {
            new_bytes.splice(0..0, BOM);
        }
        let backup = make_backup(&full, &data_dir.join("backups"), 50, 0.0, clock);
        if let Some(b) = &backup {
            out.backups.push(b.to_string_lossy().into_owned());
        }
        atomic_write_bytes(&full, &new_bytes, None, clock).map_err(|e| write_failed(&e))?;
        let check = rpy::parse_bytes(&std::fs::read(&full).unwrap_or_default(), pack);
        let old_entries = &fi.parsed.entries;
        if check.entries.len() != old_entries.len() || old_entries.iter().zip(&check.entries).any(|(a, b)| a.target != b.target) {
            let _ = atomic_write_bytes(&full, &data, None, clock);
            return fail(msg("{rel}: the check after writing failed, the file was restored to its previous state.", &[("rel", json!(rel))]));
        }
        out.fixed += items.len();
        if let Some(b) = backup {
            record.push(record_json(&rel, &b, &new_bytes, items.len()));
        }
    }
    if !record.is_empty() {
        push(data_dir, json!({"time": now_text(clock), "label": msg("Fix of speakers from the original", &[]).text(), "files": record}), clock);
    }
    Ok(out)
}

fn stack(data_dir: &Path) -> Vec<Value> {
    let Ok(text) = std::fs::read_to_string(data_dir.join(LAST_FILE)) else { return Vec::new() };
    let Ok(data) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let list = match data {
        Value::Array(a) => a,
        obj @ Value::Object(_) => vec![obj], // older format: a single record
        _ => return Vec::new(),
    };
    list.into_iter().filter(|r| r.as_object().is_some_and(|o| o.get("files").is_some_and(crate::util::truthy))).collect()
}

/// `json.dumps(value, ensure_ascii=False, indent=1)`
fn dumps_indent1(v: &Value) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b" "));
    let _ = v.serialize(&mut ser);
    String::from_utf8_lossy(&buf).into_owned()
}

fn push(data_dir: &Path, rec: Value, clock: &dyn Clock) {
    let mut all = stack(data_dir);
    all.push(rec);
    let keep = all.split_off(all.len().saturating_sub(STACK_MAX));
    let _ = atomic_write_bytes(&data_dir.join(LAST_FILE), dumps_indent1(&Value::Array(keep)).as_bytes(), None, clock);
}

/// The newest change that can still be undone: {time, label, files, lines, depth} or `None`.
pub fn last(data_dir: &Path) -> Option<Value> {
    let all = stack(data_dir);
    let mut rec = all.last()?.as_object()?.clone();
    let lines: i64 = rec["files"].as_array().map(|f| f.iter().map(|x| x.get("lines").and_then(Value::as_i64).unwrap_or(0)).sum()).unwrap_or(0);
    rec.insert("lines".into(), json!(lines));
    rec.insert("depth".into(), json!(all.len()));
    Some(Value::Object(rec))
}

/// Put the files of the newest change back from their backups. Refuses when a file was edited since (the backup would silently throw those edits away).
/// Returns {restored: [rel...], lines, label}.
pub fn undo(project: &Project, data_dir: &Path, clock: &dyn Clock) -> Res<Value> {
    let all = stack(data_dir);
    let Some(rec) = all.last() else { return fail(msg("Nothing to undo.", &[])) };
    let files = rec["files"].as_array().cloned().unwrap_or_default();
    let mut checked: Vec<(PathBuf, Vec<u8>, String)> = Vec::new();
    for f in &files {
        let file = f["file"].as_str().unwrap_or("").to_string();
        let full = project.path_of(&file).map_err(|m| Box::new(FixError(m)))?;
        let bak = PathBuf::from(f["backup"].as_str().unwrap_or(""));
        let (now, before) = match (std::fs::read(&full), std::fs::read(&bak)) {
            (Ok(n), Ok(b)) => (sha1_hex(&n), b),
            (Err(e), _) | (_, Err(e)) => {
                return fail(msg("{file}: the file or the backup cannot be read ({reason}).", &[("file", json!(file)), ("reason", json!(e.to_string()))]));
            }
        };
        if Some(now.as_str()) != f["after"].as_str() {
            return fail(msg(
                "{file}: the file changed elsewhere since the last change (e.g. in the editor). Undoing would delete later edits – the backup is in {folder}.",
                &[("file", json!(file)), ("folder", json!(bak.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()))],
            ));
        }
        checked.push((full, before, file));
    }
    for (full, before, _) in &checked {
        atomic_write_bytes(full, before, None, clock).map_err(|e| write_failed(&e))?;
    }
    let rest = &all[..all.len() - 1];
    if rest.is_empty() {
        let _ = std::fs::remove_file(data_dir.join(LAST_FILE));
    } else {
        let _ = atomic_write_bytes(&data_dir.join(LAST_FILE), dumps_indent1(&Value::Array(rest.to_vec())).as_bytes(), None, clock);
    }
    let lines: i64 = files.iter().map(|x| x.get("lines").and_then(Value::as_i64).unwrap_or(0)).sum();
    Ok(json!({"restored": checked.iter().map(|c| c.2.clone()).collect::<Vec<_>>(), "lines": lines, "label": rec.get("label").cloned().unwrap_or(json!(""))}))
}

// ---------------------------------------------------------------- editing one translated text

/// The raw text that goes between the quotes of a Ren'Py string must stay one valid string literal.
pub fn check_literal(text: &str) -> Res<()> {
    if text.contains('\n') || text.contains('\r') {
        return fail(msg("The text must not contain a real line break. Write a new line as \n.", &[]));
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                if i + 1 >= chars.len() {
                    return fail(msg("The text must not end with a lone backslash “\\” (write \\ or \n).", &[]));
                }
                i += 2;
                continue;
            }
            '"' => return fail(msg("Write a quote in the text as \\\" – a bare quote would end the string.", &[])),
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

struct Staged {
    rel: String,
    full: PathBuf,
    data: Vec<u8>,
    new: Vec<u8>,
    changes: BTreeMap<usize, String>,
    old: Vec<Entry>,
}

/// Prepare the new bytes of one file for {line: new_text}. Nothing is written.
fn stage_file(project: &Project, pack: &Pack, rel: &str, changes: &BTreeMap<usize, String>) -> Res<Staged> {
    let Some(fi) = project.files.get(rel) else { return fail(msg("{rel}: unknown file.", &[("rel", json!(rel))])) };
    let ents: BTreeMap<usize, &Entry> = project.entries(rel).iter().map(|e| (e.line, e)).collect();
    let full = project.path_of(rel).map_err(|m| Box::new(FixError(m)))?;
    let data = std::fs::read(&full).map_err(|e| cannot_read(rel, &e))?;
    if rpy::parse_bytes(&data, pack).digest != fi.parsed.digest {
        return fail(msg(
            "{rel}: the file changed on the disk since it was loaded (maybe in the editor). Refresh the files and try again.",
            &[("rel", json!(rel))],
        ));
    }
    let had_bom = data.starts_with(&BOM);
    let Some(text) = decode_strict(if had_bom { &data[BOM.len()..] } else { &data }) else {
        return fail(msg("{rel}: the file is not valid UTF-8, I rather do not edit it.", &[("rel", json!(rel))]));
    };
    let mut parts = split_lines_keeping(&text);
    for (line, new_text) in changes {
        let Some(ent) = ents.get(line) else {
            return fail(msg("{rel}:{line}: the line is no longer in the file – refresh the files.", &[("rel", json!(rel)), ("line", json!(line))]));
        };
        check_literal(new_text)?;
        let idx = 2 * (line.max(&1) - 1);
        if idx >= parts.len() || py_strip(&parts[idx]) != ent.cur_raw {
            return fail(msg("{rel}:{line}: the line no longer matches the loaded state. Refresh the files.", &[("rel", json!(rel)), ("line", json!(line))]));
        }
        let raw = parts[idx].clone();
        let Some(m) = rpy::string_re().find(&raw) else {
            return fail(msg("{rel}:{line}: there is no text in quotes on the line.", &[("rel", json!(rel)), ("line", json!(line))]));
        };
        parts[idx] = format!("{}\"{}\"{}", &raw[..m.start()], new_text, &raw[m.end()..]);
    }
    let mut new = parts.concat().into_bytes();
    if had_bom {
        new.splice(0..0, BOM);
    }
    Ok(Staged { rel: rel.to_string(), full, data, new, changes: changes.clone(), old: fi.parsed.entries.clone() })
}

/// The file on disk must differ from the old one in exactly the planned lines and nowhere else.
fn verify(staged: &Staged, written: &rpy::ParsedFile) -> bool {
    if written.entries.len() != staged.old.len() {
        return false;
    }
    staged.old.iter().zip(&written.entries).all(|(a, b)| {
        let want = staged.changes.get(&a.line).unwrap_or(&a.target);
        b.line == a.line && b.target == *want && b.who == a.who && b.suffix == a.suffix
    })
}

/// One changed line: (file, line, key, old text, new text).
pub type Detail = (String, usize, (String, String), String, String);

/// What `apply_edits` did.
#[derive(Debug, Default)]
pub struct Edited {
    pub changed: usize,
    pub files: usize,
    pub backups: Vec<String>,
    /// (file, line, key, old, new)
    pub details: Vec<Detail>,
}

impl Edited {
    pub fn to_json(&self) -> Value {
        json!({
            "changed": self.changed, "files": self.files, "backups": self.backups,
            "details": self.details.iter().map(|(f, l, k, o, n)| json!({"file": f, "line": l, "key": [k.0, k.1], "old": o, "new": n})).collect::<Vec<_>>(),
        })
    }
}

/// Replace the translated text of several lines (possibly in several files) as ONE undoable step. `edits` = [(rel, line, new_text)]. Everything else in the files stays byte for
/// byte as it was. Every file is backed up, written atomically and verified; if anything fails the files already written are restored.
pub fn apply_edits(project: &Project, pack: &Pack, data_dir: &Path, edits: &[(String, usize, String)], label: &Message, clock: &dyn Clock) -> Res<Edited> {
    let mut by_file: BTreeMap<String, BTreeMap<usize, String>> = BTreeMap::new();
    for (rel, line, new_text) in edits {
        let ent = if project.files.contains_key(rel) { project.entries(rel).iter().find(|e| e.line == *line) } else { None };
        if ent.is_some_and(|e| e.target == *new_text) {
            continue; // nothing to do for this line
        }
        by_file.entry(rel.clone()).or_default().insert(*line, new_text.clone());
    }
    if by_file.is_empty() {
        return Ok(Edited::default());
    }
    let staged: Vec<Staged> = by_file.iter().map(|(rel, ch)| stage_file(project, pack, rel, ch)).collect::<Res<_>>()?; // validates everything first
    let (mut written, mut record, mut backups): (Vec<&Staged>, Vec<Value>, Vec<String>) = (Vec::new(), Vec::new(), Vec::new());
    let result: Res<()> = (|| {
        for st in &staged {
            let backup = make_backup(&st.full, &data_dir.join("backups"), 50, 0.0, clock);
            atomic_write_bytes(&st.full, &st.new, None, clock).map_err(|e| write_failed(&e))?;
            written.push(st);
            let check = rpy::parse_bytes(
                &std::fs::read(&st.full).map_err(|e| Box::new(FixError(msg("The write failed: {reason}", &[("reason", json!(e.to_string()))]))))?,
                pack,
            );
            if !verify(st, &check) {
                return fail(msg("{rel}: the check after writing failed, the changes were restored to their previous state.", &[("rel", json!(st.rel))]));
            }
            if let Some(b) = backup {
                backups.push(b.to_string_lossy().into_owned());
                record.push(record_json(&st.rel, &b, &st.new, st.changes.len()));
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        for st in written {
            let _ = atomic_write_bytes(&st.full, &st.data, None, clock); // roll back what was already written
        }
        return Err(e);
    }
    if !record.is_empty() {
        push(data_dir, json!({"time": now_text(clock), "label": label.text(), "files": record}), clock);
    }
    let mut details = Vec::new();
    for st in &staged {
        for e in st.old.iter().filter(|e| st.changes.contains_key(&e.line)) {
            details.push((st.rel.clone(), e.line, e.key(), e.target.clone(), st.changes[&e.line].clone()));
        }
    }
    Ok(Edited { changed: staged.iter().map(|s| s.changes.len()).sum(), files: staged.len(), backups, details })
}

/// Replace the translated text of one unit. Returns (changed, backup).
pub fn edit_translation(project: &Project, pack: &Pack, data_dir: &Path, rel: &str, line: usize, new_text: &str, clock: &dyn Clock) -> Res<Value> {
    let Some(ent) = project.entries(rel).iter().find(|e| e.line == line) else {
        return fail(msg("The line is no longer in the file – refresh the files.", &[]));
    };
    check_literal(new_text)?;
    if new_text == ent.target {
        return Ok(json!({"changed": false}));
    }
    let name = Path::new(rel).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let label = msg("Editing line {line} in {name}", &[("line", json!(line)), ("name", json!(name))]);
    let res = apply_edits(project, pack, data_dir, &[(rel.to_string(), line, new_text.to_string())], &label, clock)?;
    let mut out = Map::new();
    out.insert("changed".into(), json!(true));
    out.insert("backup".into(), json!(res.backups.first().cloned().unwrap_or_default()));
    out.insert("details".into(), res.to_json()["details"].clone());
    Ok(Value::Object(out))
}
