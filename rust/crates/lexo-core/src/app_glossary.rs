//! Port of `core/lexo/app_glossary.py`: glossary of names: speakers and terms, import from the game's character definitions.
use crate::api::{ok, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, character_re, Res, CHARACTERS_MAX_BYTES, SCREEN_KEYWORDS};
use crate::gametools::{keys, rglob_suffix, sort_paths};
use crate::messages::{msg, py_str};
use crate::store::{Gloss, Kind, Label, GLOSSARY_TYPES};
use crate::util::{py_splitlines, py_strip, truthy};
use crate::validator;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Lines of a text file read like `read_text(encoding="utf-8-sig", errors="replace")`.
fn read_sig(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    Ok(String::from_utf8_lossy(body).into_owned())
}

/// A file name or folder name in lower case (what Python's `d.lower()` gives).
fn low(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn sort_by_text(v: &mut [PathBuf]) {
    v.sort_by_key(|p| p.to_string_lossy().to_lowercase());
}

/// Does the script have at least one `define x = Character(...)` line? (a quick text test, no parsing)
fn defines_characters(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(m) if m.len() <= CHARACTERS_MAX_BYTES => {}
        _ => return false,
    }
    match read_sig(path) {
        Ok(t) => py_splitlines(&t).iter().any(|l| character_re().is_match(l)),
        Err(_) => false,
    }
}

/// `os.walk(base)`: every file below `base`, folders named in `skip` (lower case) left out.
fn walk_files(base: &Path, skip: &[&str]) -> Vec<PathBuf> {
    fn go(dir: &Path, skip: &[&str], out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        let (mut dirs, mut files) = (Vec::new(), Vec::new());
        for e in entries {
            if e.path().is_dir() {
                if !skip.contains(&e.file_name().to_string_lossy().to_lowercase().as_str()) {
                    dirs.push(e.path());
                }
            } else {
                files.push(e.path());
            }
        }
        out.extend(files);
        for d in dirs {
            go(&d, skip, out);
        }
    }
    let mut out = Vec::new();
    go(base, skip, &mut out);
    out
}

/// `os.path.commonpath` of folders (the parts they share from the start).
fn common_path(paths: &[PathBuf]) -> Option<PathBuf> {
    let first = paths.first()?;
    let mut common: Vec<_> = first.components().collect();
    for p in &paths[1..] {
        let parts: Vec<_> = p.components().collect();
        let n = common.iter().zip(&parts).take_while(|(a, b)| keys(Path::new(a.as_os_str())) == keys(Path::new(b.as_os_str()))).count();
        common.truncate(n);
    }
    if common.is_empty() {
        return None;
    }
    Some(common.iter().map(|c| c.as_os_str()).collect())
}

impl App {
    /// Glossary rows: every speaker code found in the project (with its line count and an example) merged with the saved names, plus saved entries that are not speakers (terms).
    pub fn glossary_payload(&self) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        struct Found {
            count: i64,
            example: String,
            file: String,
            line: usize,
        }
        let mut found: IndexMap<String, Found> = IndexMap::new();
        let mut rels: Vec<&String> = o.project.files.keys().collect();
        rels.sort_by_key(|r| r.to_lowercase());
        for rel in rels {
            for e in o.project.entries(rel) {
                let code = App::speaker_code(&e.who);
                if e.unit == "dialogue" && !code.is_empty() && !SCREEN_KEYWORDS.contains(&code.as_str()) {
                    let f = found.entry(code).or_insert_with(|| Found { count: 0, example: String::new(), file: rel.clone(), line: e.line });
                    f.count += 1;
                    if f.example.is_empty() {
                        if let Some(s) = e.source.as_deref().filter(|s| !s.is_empty()) {
                            f.example = s.chars().take(90).collect();
                        }
                    }
                }
            }
        }
        let mut items: Vec<Map<String, Value>> = Vec::new();
        let obj = |v: Value| v.as_object().cloned().unwrap_or_default();
        for (code, f) in &found {
            let g = o.store.glossary.get(code);
            items.push(obj(json!({
                "code": code, "name": g.map_or("", |g| g.n.as_str()), "type": g.map_or("speaker", |g| g.t.as_str()), "note": g.map_or("", |g| g.c.as_str()), "count": f.count,
                "example": f.example, "file": f.file, "line": f.line, "in_project": true,
            })));
        }
        for (code, g) in &o.store.glossary {
            if !found.contains_key(code) {
                items.push(obj(
                    json!({"code": code, "name": g.n, "type": g.t, "note": g.c, "count": 0, "example": "", "file": "", "line": 0, "in_project": false}),
                ));
            }
        }
        items.sort_by_key(|i| (-i["count"].as_i64().unwrap_or(0), i["code"].as_str().unwrap_or("").to_lowercase()));
        let names: Map<String, Value> =
            o.store.glossary.iter().filter(|(_, g)| !g.n.is_empty() && g.t == "speaker").map(|(c, g)| (c.clone(), json!(g.n))).collect();
        let unnamed = items.iter().filter(|i| i["type"] == "speaker" && i["in_project"] == true && i["name"] == "").count();
        Ok(json!({"items": items, "names": names, "types": GLOSSARY_TYPES, "unnamed": unnamed}))
    }

    /// Create / update entries. An entry with neither name nor note is removed. One undo step.
    pub fn glossary_set(&mut self, items: &[Value]) -> Res<Value> {
        self.need_project()?;
        let mut clean: Vec<(String, String, String, String)> = Vec::new();
        for it in items {
            let text = |k: &str| py_strip(&it.get(k).map(py_str).unwrap_or_default()).to_string();
            let code = text("code");
            if code.is_empty() || code.chars().count() > 60 || code.contains(['\r', '\n']) {
                return Err(app_err(msg("Invalid code: {code}", &[("code", json!(code))])));
            }
            let typ = it.get("type").cloned().unwrap_or_else(|| json!("speaker"));
            let Some(typ) = typ.as_str().filter(|t| GLOSSARY_TYPES.contains(t)) else {
                return Err(app_err(msg("Invalid type of a glossary item.", &[])));
            };
            let (name, note) = (text("name"), text("note"));
            if name.chars().count() > 200 || note.chars().count() > 500 {
                return Err(app_err(msg("The name or note is too long.", &[])));
            }
            clean.push((code, name, typ.to_string(), note));
        }
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        o.store.change(
            Label::Msg(msg("Glossary ({n})", &[("n", json!(clean.len()))])),
            &[Kind::Glossary],
            |st| {
                for (code, name, typ, note) in &clean {
                    if !name.is_empty() || !note.is_empty() {
                        st.glossary.insert(code.clone(), Gloss { n: name.clone(), t: typ.clone(), c: note.clone() });
                    } else {
                        st.glossary.shift_remove(code);
                    }
                }
            },
            clock,
        )?;
        let mut out = json!({"ok": true});
        if let (Some(o), Some(p)) = (out.as_object_mut(), self.glossary_payload()?.as_object()) {
            o.extend(p.clone());
        }
        Ok(out)
    }

    /// All files of the game that define characters: the files named `characters*.rpy` and, when there are none, every other script that has a `define x = Character(...)`
    /// line. The translation folder is usually `<game>/game/tl/<lang>`, so the game folder is searched upwards.
    pub fn find_characters_files(&self) -> Vec<PathBuf> {
        let skip = ["tl", "cache", "saves", "_progress_data", ".aside", ".lexo", ".git"];
        let Some(base) = self.game_dir().filter(|b| b.is_dir()) else { return Vec::new() };
        let (mut named, mut scripts): (Vec<PathBuf>, Vec<PathBuf>) = (Vec::new(), Vec::new());
        for f in walk_files(&base, &skip) {
            let name = low(&f);
            if name.ends_with(".rpy") {
                if name.starts_with("characters") {
                    named.push(f);
                } else {
                    scripts.push(f);
                }
            }
        }
        if !named.is_empty() {
            sort_by_text(&mut named);
            return named; // Ren'Py loads files in path order
        }
        let mut defining: Vec<PathBuf> = scripts.into_iter().filter(|f| defines_characters(f)).collect();
        sort_by_text(&mut defining);
        defining
    }

    /// Read `define xx = Character("Name", ...)` lines of the game's character files and compare with the glossary. Nothing is written. `path` may be a file, a folder (all
    /// `characters*.rpy` below it) or empty (found automatically). Ren'Py rule: a later definition of the same code replaces an earlier one, so the last one wins; codes
    /// defined several times with different names are reported in `duplicates`. States: new | same | conflict (another name is already stored) | dynamic.
    pub fn glossary_import_preview(&self, path: &str) -> Res<Value> {
        self.need_project()?;
        let given = py_strip(path).to_string();
        let files: Vec<PathBuf> = if !given.is_empty() {
            let target = expand_user(&given);
            if target.is_dir() {
                let relparts = |p: &Path| -> Vec<String> {
                    p.strip_prefix(&target).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect()
                };
                let mut files: Vec<PathBuf> = rglob_prefix(&target, "characters").into_iter().filter(|p| !relparts(p).iter().any(|x| x == "tl")).collect();
                sort_by_text(&mut files);
                if files.is_empty() {
                    files = rglob_suffix(&target, &[".rpy"])
                        .into_iter()
                        .filter(|p| !relparts(p).iter().any(|x| ["tl", "cache", "saves"].contains(&x.as_str())) && defines_characters(p))
                        .collect();
                    sort_by_text(&mut files);
                }
                files
            } else {
                vec![target]
            }
        } else {
            self.find_characters_files()
        };
        if files.is_empty() {
            return Ok(json!({"path": given, "files": [], "items": [], "missing": [], "duplicates": [],
                "message": msg("No character definitions were found – give the path to a file or to the game folder.", &[]).wire()}));
        }
        // code -> list of (name, dynamic)
        let mut defs: IndexMap<String, Vec<(String, bool, PathBuf)>> = IndexMap::new();
        for f in &files {
            if f.extension().map(|x| x.to_string_lossy().to_lowercase()) != Some("rpy".into()) {
                return Err(app_err(msg("Only .rpy files (characters.rpy) can be imported.", &[])));
            }
            if !f.is_file() {
                return Err(app_err(msg("The file does not exist: {path}", &[("path", json!(f.to_string_lossy()))])));
            }
            if std::fs::metadata(f).map_or(0, |m| m.len()) > CHARACTERS_MAX_BYTES {
                return Err(app_err(msg(
                    "The file is too large for a list of characters: {name}",
                    &[("name", json!(f.file_name().unwrap_or_default().to_string_lossy()))],
                )));
            }
            let text = read_sig(f).map_err(|e| app_err(msg("The file cannot be read: {reason}", &[("reason", json!(e.to_string()))])))?;
            for line in py_splitlines(&text) {
                if let Some(m) = character_re().captures(&line) {
                    let raw = m.get(3).or_else(|| m.get(4)).map_or("", |g| g.as_str());
                    let name = py_strip(&raw.replace("\\\"", "\"")).to_string();
                    let dynamic = m.get(2).is_some() || name.starts_with('[');
                    defs.entry(m[1].to_string()).or_default().push((name, dynamic, f.clone()));
                }
            }
        }
        let o = self.o()?;
        let mut in_project: BTreeSet<String> = BTreeSet::new();
        for rel in o.project.files.keys() {
            for e in o.project.entries(rel) {
                if e.unit == "dialogue" {
                    in_project.insert(App::speaker_code(&e.who));
                }
            }
        }
        in_project.retain(|c| !SCREEN_KEYWORDS.contains(&c.as_str()));
        let parents: Vec<PathBuf> = files.iter().map(|f| f.parent().map(Path::to_path_buf).unwrap_or_default()).collect();
        let game_dir = common_path(&parents);
        let (mut items, mut duplicates): (Vec<Value>, Vec<Value>) = (Vec::new(), Vec::new());
        for (code, versions) in &defs {
            let (name, dynamic, src) = versions.last().unwrap().clone();
            let mut names: Vec<String> = Vec::new();
            for (n, _, _) in versions {
                if !n.is_empty() && !names.contains(n) {
                    names.push(n.clone());
                }
            }
            if names.len() > 1 {
                duplicates.push(json!({"code": code, "names": names, "used": name}));
            }
            let cur = o.store.glossary.get(code).map(|g| g.n.clone()).unwrap_or_default();
            let (state, name) = if dynamic {
                ("dynamic", String::new())
            } else if name.is_empty() {
                continue;
            } else if cur.is_empty() {
                ("new", name)
            } else if cur == name {
                ("same", name)
            } else {
                ("conflict", name)
            };
            let shown = match &game_dir {
                Some(g) => src
                    .strip_prefix(g)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| src.file_name().unwrap_or_default().to_string_lossy().into_owned()),
                None => src.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            };
            let note = if dynamic {
                msg("the name is entered by the player (DynamicCharacter)", &[]).wire()
            } else if names.len() > 1 {
                msg("defined several times ({names}), the last one counts", &[("names", json!(names.join(" / ")))]).wire()
            } else {
                json!("")
            };
            items.push(
                json!({"code": code, "name": name, "current": cur, "state": state, "in_project": in_project.contains(code), "note": note, "source": shown}),
            );
        }
        let mut missing: Vec<&String> = in_project.iter().filter(|c| !c.is_empty() && !defs.contains_key(*c)).collect();
        missing.sort();
        let shown_path = if given.is_empty() {
            game_dir.clone().unwrap_or_else(|| files[0].parent().map(Path::to_path_buf).unwrap_or_default()).to_string_lossy().into_owned()
        } else {
            given
        };
        Ok(
            json!({"path": shown_path, "files": files.iter().map(|f| f.to_string_lossy()).collect::<Vec<_>>(), "items": items, "missing": missing, "duplicates": duplicates, "message": ""}),
        )
    }

    /// Store the reviewed names (a stored note is kept). One undo step.
    pub fn glossary_import_apply(&mut self, items: &[Value]) -> Res<Value> {
        self.need_project()?;
        let mut out: Vec<Value> = Vec::new();
        for it in items {
            let code = py_strip(&it.get("code").map(py_str).unwrap_or_default()).to_string();
            let cur_note = self.o()?.store.glossary.get(&code).map(|g| g.c.clone()).unwrap_or_default();
            let note = if cur_note.is_empty() { it.get("note").map(py_str).unwrap_or_default() } else { cur_note };
            out.push(json!({"code": code, "name": py_strip(&it.get("name").map(py_str).unwrap_or_default()), "type": "speaker", "note": note}));
        }
        self.glossary_set(&out)
    }

    pub fn glossary_delete(&mut self, codes: &[Value]) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let list: Vec<String> = codes.iter().map(py_str).collect();
        o.store.change(
            Label::Msg(msg("Glossary: deleted ({n})", &[("n", json!(codes.len()))])),
            &[Kind::Glossary],
            |st| {
                for c in &list {
                    st.glossary.shift_remove(c);
                }
            },
            clock,
        )?;
        let mut out = json!({"ok": true});
        if let (Some(o), Some(p)) = (out.as_object_mut(), self.glossary_payload()?.as_object()) {
            o.extend(p.clone());
        }
        Ok(out)
    }

    /// Every line whose original contains the glossary term `code`, with whether the Czech form is used.
    pub fn glossary_occurrences(&self, code: &str, only_missing: bool, limit: usize) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let Some(g) = o.store.glossary.get(code) else { return Err(app_err(msg("The glossary entry does not exist.", &[]))) };
        let czech = g.n.clone();
        let (mut rows, mut found, mut missing, mut untranslated) = (Vec::new(), 0, 0, 0);
        let mut rels: Vec<&String> = o.project.files.keys().collect();
        rels.sort_by_key(|r| r.to_lowercase());
        for rel in rels {
            for e in o.project.entries(rel) {
                let Some(src) = e.source.as_deref().filter(|s| validator::term_in_text(s, code)) else { continue };
                let translated = o.project.is_translated(&o.store, rel, e);
                let used: Option<bool> = (!czech.is_empty() && translated).then(|| validator::term_used(&e.target, &czech, &o.pack));
                match used {
                    Some(true) => found += 1,
                    Some(false) => missing += 1,
                    None => untranslated += 1,
                }
                if only_missing && used != Some(false) {
                    continue;
                }
                rows.push(json!({"file": rel, "line": e.line, "who": e.who, "source": src, "target": e.target, "translated": translated, "used": used}));
            }
        }
        let truncated = rows.len() > limit;
        rows.truncate(limit);
        Ok(json!({"code": code, "czech": czech, "found": found, "missing": missing, "untranslated": untranslated, "items": rows, "truncated": truncated}))
    }
}

fn expand_user(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix('~') {
        if rest.is_empty() || rest.starts_with(['/', '\\']) {
            if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
                return PathBuf::from(home).join(rest.trim_start_matches(['/', '\\']));
            }
        }
    }
    PathBuf::from(p)
}

/// `target.rglob("<prefix>*.rpy")` (the case of the name does not matter on Windows).
fn rglob_prefix(target: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = rglob_suffix(target, &[".rpy"])
        .into_iter()
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if cfg!(windows) {
                n.to_lowercase().starts_with(prefix)
            } else {
                n.starts_with(prefix)
            }
        })
        .collect();
    sort_paths(&mut v);
    v
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_glossary(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.glossary_payload()?)
}

pub fn op_glossary_set(app: &mut App, a: &Args) -> Res<Answer> {
    crate::api::plain(app.glossary_set(a.l("items"))?)
}

pub fn op_glossary_import_preview(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.glossary_import_preview(a.s("path"))?)
}

pub fn op_glossary_import_apply(app: &mut App, a: &Args) -> Res<Answer> {
    crate::api::plain(app.glossary_import_apply(a.l("items"))?)
}

pub fn op_glossary_delete(app: &mut App, a: &Args) -> Res<Answer> {
    crate::api::plain(app.glossary_delete(a.l("codes"))?)
}

pub fn op_glossary_occurrences(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.glossary_occurrences(a.s("code"), a.f("missing"), 400)?)
}

#[allow(dead_code)]
fn unused() -> bool {
    truthy(&Value::Null)
}
