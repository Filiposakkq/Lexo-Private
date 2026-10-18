//! Port of `core/lexo/missed.py`: texts that Ren'Py's `translate` command does not extract, found by reading the game's scripts.
//!
//! The generator only takes what goes through the translation system with a block of its own. A text shown some other way (`show text "..."`, `renpy.input("prompt")`, `renpy.notify(...)`,
//! a title assigned to a variable and shown later, or dialogue in a place the generator does not reach) has no block, yet it can be translated with a `translate <lang> strings:` block.
//! This module finds such texts and writes them into a strings file. It is a heuristic: every finding has a `level` (`likely` or `maybe`) and a `kind` (`text` or `dialogue`). Menu
//! choices and everything inside `screen` blocks are skipped, because Ren'Py extracts those itself.
use crate::fsutil::{atomic_write_text, WriteFailed};
use crate::util::{py_splitlines, py_strip};
use fancy_regex::Regex as FancyRegex;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const SKIP_DIRS: [&str; 8] = ["tl", "cache", "saves", ".aside", ".lexo", "_progress_data", ".git", "__pycache__"];
const STRING: &str = r#""(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'"#;
/// Words that start a Ren'Py statement: a line beginning with one of them is not a dialogue line, whatever follows.
const NOT_A_SPEAKER: [&str; 32] = [
    "show",
    "hide",
    "scene",
    "play",
    "queue",
    "stop",
    "window",
    "pause",
    "nvl",
    "call",
    "jump",
    "return",
    "with",
    "python",
    "init",
    "define",
    "default",
    "if",
    "elif",
    "else",
    "while",
    "for",
    "menu",
    "label",
    "screen",
    "image",
    "transform",
    "style",
    "voice",
    "camera",
    "pass",
    "renpy",
];
const STATEMENT_SKIP: [&str; 25] = [
    "translate ",
    "image ",
    "transform ",
    "style ",
    "label ",
    "jump ",
    "call ",
    "play ",
    "queue ",
    "stop ",
    "scene ",
    "hide ",
    "with ",
    "from ",
    "import ",
    "init offset",
    "python hide",
    "define build.",
    "define config.",
    "define gui.",
    "build.",
    "config.",
    "gui.",
    "$ config.",
    "$ gui.",
];
/// Top-level statements whose indented body is not text for the player (image definitions, transforms, styles, translations).
const BLOCK_SKIP: [&str; 6] = ["image ", "transform ", "style ", "translate ", "layeredimage ", "init python in "];
const SINK_NAMES: &str =
    r"show\s+text|renpy\.input|renpy\.notify|renpy\.confirm|renpy\.say|renpy\.display_say|Notify|Text|ui\.text|renpy\.show_screen|tooltip|[A-Za-z_]\w*\.say";
/// Callables that take a string but are not text for the player: never suggested as "functions that show text".
const NOT_TEXT_FUNCTIONS: [&str; 48] = [
    "Character",
    "DynamicCharacter",
    "Image",
    "Transform",
    "Solid",
    "Color",
    "Movie",
    "Play",
    "Queue",
    "Stop",
    "Show",
    "Hide",
    "Jump",
    "Call",
    "SetVariable",
    "SetField",
    "ToggleVariable",
    "Function",
    "Return",
    "Action",
    "Fixed",
    "Frame",
    "LiveComposite",
    "Null",
    "AlphaMask",
    "ConditionSwitch",
    "DynamicImage",
    "FileSave",
    "FileLoad",
    "OpenURL",
    "Preference",
    "Notify",
    "Text",
    "SetDict",
    "getattr",
    "setattr",
    "hasattr",
    "isinstance",
    "len",
    "str",
    "int",
    "float",
    "print",
    "open",
    "range",
    "list",
    "dict",
    "set",
];
const LIST_METHODS: [&str; 12] = ["append", "remove", "insert", "extend", "add", "update", "discard", "setdefault", "index", "count", "get", "pop"];
const DATA_START: [&str; 5] = ["init", "python", "define ", "default ", "$ "];

struct Patterns {
    literal: Regex,
    say: Regex,
    choice: Regex,
    call: Regex,
    noise_name: Regex,
    name_hint: FancyRegex,
    character_def: Regex,
    pathlike: FancyRegex,
    ident: Regex,
    color: Regex,
    tag: Regex,
    comment_say: Regex,
    refer: Regex,
    first_word: Regex,
    letters3: Regex,
    func_name: Regex,
}

fn p() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        literal: Regex::new(r#""((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'"#).unwrap(),
        say: Regex::new(&format!(r"^\s*(?:[A-Za-z_]\w*(?:\s+[\w.@]+)*\s+)?(?:{STRING})(?:\s+(?:with|id)\s+\S+)*\s*$")).unwrap(),
        choice: Regex::new(&format!(r"^\s*(?:{STRING})(?:\s+if\s+.+)?:\s*$")).unwrap(),
        call: Regex::new(&format!(r"([A-Za-z_][\w.]*)\s*\(\s*(?:{STRING})")).unwrap(),
        noise_name: Regex::new(r"(?:Error|Exception|Warning)$|^self\.|\.(?:write|log|debug|info|warning|error|print|join|format|get|pop|startswith|endswith|replace|split|say)$|_indent$").unwrap(),
        name_hint: FancyRegex::new(r"(?i)\w*(?:title|name|text|message|msg|caption|label|desc|prompt|hint|tip|header|heading)\w*\s*=(?!=)").unwrap(),
        character_def: Regex::new(r"\bCharacter\s*\(").unwrap(),
        pathlike: FancyRegex::new(r#"(?i)/|\\(?![ntr"'\\])|\.(?:png|jpg|jpeg|webp|webm|mp4|mp3|ogg|opus|wav|rpy|rpyc|ttf|otf|py|json|txt)\b"#).unwrap(),
        ident: Regex::new(r"^[\w.\[\]]+$").unwrap(),
        color: Regex::new(r"^#[0-9a-fA-F]{3,8}$").unwrap(),
        tag: Regex::new(r"\{[^{}]*\}|\[[^\[\]]*\]").unwrap(),
        comment_say: Regex::new(&format!(r"^\s*#\s*(?:[A-Za-z_]\w*(?:\s+[\w.@]+)*\s+)?({STRING})")).unwrap(),
        refer: Regex::new(r"^#\s*(?:game/)?(?P<file>[^:]+?\.rpym?):(?P<line>\d+)\s*$").unwrap(),
        first_word: Regex::new(r"^[A-Za-z_]\w*").unwrap(),
        letters3: Regex::new(r"[A-Za-z]{3,}").unwrap(),
        func_name: Regex::new(r"\A[A-Za-z_][\w.]*\z").unwrap(),
    })
}

/// The pattern for "this text is shown to the player": the built-in functions plus the project's own (`functions`, e.g. `["notify", "popup"]`).
pub fn sink_regex(functions: &[String]) -> Regex {
    let extra: Vec<String> = functions.iter().filter(|f| !f.is_empty() && p().func_name.is_match(py_strip(f))).map(|f| regex::escape(py_strip(f))).collect();
    let more = if extra.is_empty() { String::new() } else { format!("|{}", extra.join("|")) };
    Regex::new(&format!(r"\b(?:{SINK_NAMES}{more})\b")).unwrap()
}

/// Does this string look like something a person reads (not an id, a path, a colour or a single lower-case word)?
pub fn looks_like_text(text: &str) -> bool {
    let plain = p().tag.replace_all(text, " ").into_owned(); // {i} and {/i} must not look like a path
    if text.chars().count() < 3 || p().color.is_match(text) || p().pathlike.is_match(&plain).unwrap_or(false) {
        return false;
    }
    if !p().letters3.is_match(&plain) {
        return false;
    }
    if py_strip(text).contains(' ') {
        return true;
    }
    let first_upper = text.chars().next().is_some_and(|c| c.is_uppercase());
    if p().ident.is_match(text) && !first_upper {
        return false;
    }
    first_upper && !text.contains('_')
}

/// (relative path with `/`, absolute path) of the scripts below `game_dir` outside `tl` and the other skipped folders, in the order Python's sorted paths have on Windows.
fn scripts(game_dir: &Path) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(Vec<String>, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if e.file_name().to_string_lossy().to_lowercase().ends_with(".rpy") {
                let parts: Vec<String> = path.strip_prefix(root).unwrap_or(&path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                out.push((parts, path));
            }
        }
    }
    let mut all = Vec::new();
    walk(game_dir, game_dir, &mut all);
    all.sort_by_key(|(parts, _)| parts.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>());
    all.into_iter()
        .filter(|(parts, _)| !parts[..parts.len() - 1].iter().any(|d| SKIP_DIRS.contains(&d.as_str())))
        .map(|(parts, path)| (parts.join("/"), path))
        .collect()
}

/// Is there at least one script (outside tl and the other skipped folders) below `game_dir`?
pub fn has_scripts(game_dir: &Path) -> bool {
    !scripts(game_dir).is_empty()
}

fn literal(m: &regex::Captures) -> String {
    m.get(1).or_else(|| m.get(2)).map_or("", |g| g.as_str()).to_string()
}

fn read_lines(path: &Path, bom: bool) -> Option<Vec<String>> {
    let bytes = std::fs::read(path).ok()?;
    let body = if bom { bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes) } else { &bytes[..] };
    Some(py_splitlines(&String::from_utf8_lossy(body)))
}

/// What the translation already has: the `old "..."` strings, the original texts of its dialogue blocks (the `# who "text"` comments) and the places `(file, line)` its blocks
/// point to (the `# game/script.rpy:22` comments of Ren'Py's generator).
pub struct Known {
    pub strings: HashSet<String>,
    pub dialogue: HashSet<String>,
    pub places: HashSet<(String, usize)>,
}

fn rpy_below(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let path = e.path();
            if path.is_dir() {
                rpy_below(&path, out);
            } else if e.file_name().to_string_lossy().to_lowercase().ends_with(".rpy") {
                out.push(path);
            }
        }
    }
}

pub fn known_texts(tl_dir: &Path) -> Known {
    let mut k = Known { strings: HashSet::new(), dialogue: HashSet::new(), places: HashSet::new() };
    let mut files = Vec::new();
    rpy_below(tl_dir, &mut files);
    for path in files {
        let parts: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        if parts.iter().any(|x| x == "_progress_data" || x == ".aside" || x == ".lexo") {
            continue;
        }
        let Some(lines) = read_lines(&path, true) else { continue };
        for line in lines {
            let s = py_strip(&line);
            if s.starts_with("old ") {
                if let Some(m) = p().literal.captures(s) {
                    k.strings.insert(literal(&m));
                }
            } else if s.starts_with('#') {
                if let Some(r) = p().refer.captures(s) {
                    k.places.insert((r["file"].replace('\\', "/"), r["line"].parse().unwrap_or(0)));
                    continue;
                }
                if let Some(m) = p().comment_say.captures(s) {
                    let t = &m[1];
                    k.dialogue.insert(t[1..t.len() - 1].to_string());
                }
            }
        }
    }
    k
}

pub fn known_strings(tl_dir: &Path) -> HashSet<String> {
    known_texts(tl_dir).strings
}

/// A text in the game's scripts that the translation does not have.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub text: String,
    /// `text` or `dialogue`
    pub kind: &'static str,
    pub file: String,
    pub line: usize,
    /// `likely` or `maybe`
    pub level: &'static str,
    pub context: String,
    pub count: usize,
}

impl Found {
    pub fn to_json(&self) -> Value {
        json!({"text": self.text, "kind": self.kind, "file": self.file, "line": self.line, "level": self.level, "context": self.context, "count": self.count})
    }
}

/// Texts in the game's scripts that the translation does not have. `known` = the string table; `known_places` = the places its dialogue blocks point to (a dialogue line at any
/// other place has no block); `known_dialogue` = the texts of the dialogue blocks, used when the places are not known; with neither, dialogue is not examined. Returns (items, truncated);
/// the likely ones first. `functions`: names of the project's own functions that show text; `ignored`: texts the translator chose to leave out.
#[allow(clippy::too_many_arguments)]
pub fn find_missed(
    game_dir: &Path,
    known: &HashSet<String>,
    known_dialogue: Option<&HashSet<String>>,
    known_places: Option<&HashSet<(String, usize)>>,
    limit: usize,
    functions: &[String],
    ignored: &[String],
) -> (Vec<Found>, bool) {
    let mut found: IndexMap<String, Found> = IndexMap::new();
    let sink = sink_regex(functions);
    let skip_texts: HashSet<&String> = ignored.iter().collect();
    let places = known_places.filter(|p| !p.is_empty());
    for (rel, path) in scripts(game_dir) {
        let Some(lines) = read_lines(&path, true) else { continue };
        let (mut in_screen, mut in_skipped_block, mut in_data) = (false, false, false);
        for (idx, line) in lines.iter().enumerate() {
            let no = idx + 1;
            let s = py_strip(line);
            if s.is_empty() || s.starts_with('#') {
                continue;
            }
            let indented = line.starts_with([' ', '\t']);
            if !indented {
                in_screen = s.starts_with("screen ");
                in_skipped_block = BLOCK_SKIP.iter().any(|b| s.starts_with(b));
                in_data = DATA_START.iter().any(|b| s.starts_with(b)); // an indented line below `default x = [` or `init python:` is data, not a spoken line
            }
            if in_skipped_block {
                continue;
            }
            if in_screen && !s.starts_with('$') {
                continue; // the screen language is extracted by Ren'Py; only python lines (`$ ...`) can hide texts
            }
            if STATEMENT_SKIP.iter().any(|b| s.starts_with(b)) || p().choice.is_match(line) || p().character_def.is_match(line) {
                continue;
            }
            let first = p().first_word.find(s).map(|m| m.as_str());
            if p().say.is_match(line) && !first.is_some_and(|f| NOT_A_SPEAKER.contains(&f)) {
                if in_data && indented {
                    continue;
                }
                let last = p().literal.captures_iter(line).last().unwrap();
                let text = literal(&last);
                let without_block = if let Some(pl) = places {
                    !pl.contains(&(rel.clone(), no))
                } else if let Some(kd) = known_dialogue {
                    !kd.contains(&text)
                } else {
                    continue;
                };
                if without_block && !known.contains(&text) && !skip_texts.contains(&text) && looks_like_text(&text) {
                    add(&mut found, text, "dialogue", "likely", &rel, no, &s.chars().take(100).collect::<String>());
                }
                continue;
            }
            for m in p().literal.captures_iter(line) {
                let text = literal(&m);
                if known.contains(&text) || skip_texts.contains(&text) || !looks_like_text(&text) {
                    continue;
                }
                let before = &line[..m.get(0).unwrap().start()];
                let level = if sink.is_match(before) || p().name_hint.is_match(before).unwrap_or(false) { "likely" } else { "maybe" };
                add(&mut found, text, "text", level, &rel, no, &s.chars().take(100).collect::<String>());
            }
        }
    }
    let mut items: Vec<Found> = found.into_values().collect();
    items.sort_by_key(|i| (i.level != "likely", i.file.to_lowercase(), i.line));
    let truncated = items.len() > limit;
    items.truncate(limit);
    (items, truncated)
}

#[allow(clippy::too_many_arguments)]
fn add(found: &mut IndexMap<String, Found>, text: String, kind: &'static str, level: &'static str, rel: &str, no: usize, context: &str) {
    match found.get_mut(&text) {
        Some(old) => {
            old.count += 1;
            if level == "likely" && old.level != "likely" {
                old.level = "likely";
                old.kind = kind;
                old.context = context.to_string();
                old.file = rel.to_string();
                old.line = no;
            }
        }
        None => {
            found.insert(text.clone(), Found { text, kind, file: rel.to_string(), line: no, level, context: context.to_string(), count: 1 });
        }
    }
}

pub const FILE_NAME: &str = "extra_strings.rpy";

fn resolve_lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(|c| PathBuf::from(c.to_string_lossy().trim_start_matches(r"\\?\"))).unwrap_or_else(|_| resolve_lexical(p))
}

/// The translation file that mirrors the script `rel` (game/script2.rpy -> tl/<language>/script2.rpy) when it exists, else `None`.
fn target_file(tl_dir: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || !rel.ends_with(".rpy") {
        return None;
    }
    let path = canonical(&tl_dir.join(rel));
    (path.starts_with(tl_dir) && path != tl_dir && path.is_file()).then_some(path)
}

/// What `write_strings` did: the files written with the number of texts in each, how many were added and how many were skipped.
#[derive(Debug, PartialEq)]
pub struct Written {
    pub file: String,
    pub files: Vec<(String, usize)>,
    pub added: usize,
    pub skipped: usize,
}

/// A text chosen to be written: its text, the script it is in and the line.
pub struct Item {
    pub text: String,
    pub file: String,
    pub line: i64,
}

/// Add the chosen texts as `translate <language> strings:` blocks, with the original as the first translation (so they show up as untranslated). By default all of them go
/// to `extra_strings.rpy`; with `into_files` each goes to the translation file that mirrors its script. Texts already in the string table are not written twice.
pub fn write_strings(
    tl_dir: &Path,
    language: &str,
    items: &[Item],
    backup_dir: Option<&Path>,
    into_files: bool,
    clock: &dyn crate::clock::Clock,
) -> Result<Written, WriteFailed> {
    let tl_dir = canonical(tl_dir); // one spelling of the path
    let known = known_strings(&tl_dir);
    // the file each text goes to, in the order the files were first needed: (text, script, line)
    let mut targets: IndexMap<PathBuf, Vec<(String, String, i64)>> = IndexMap::new();
    let (mut seen, mut skipped): (HashSet<String>, usize) = (HashSet::new(), 0);
    for it in items {
        if it.text.is_empty() || known.contains(&it.text) || seen.contains(&it.text) {
            skipped += 1;
            continue;
        }
        seen.insert(it.text.clone());
        let rel = it.file.replace('\\', "/");
        let target = if into_files { target_file(&tl_dir, &rel) } else { None }.unwrap_or_else(|| tl_dir.join(FILE_NAME));
        targets.entry(target).or_default().push((it.text.clone(), rel, it.line));
    }
    let mut files: Vec<(String, usize)> = Vec::new();
    for (path, entries) in &targets {
        let old_text = if path.is_file() {
            String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).trim_start_matches('\u{feff}').to_string()
        } else {
            String::new()
        };
        let mut lines: Vec<String> = vec![format!("translate {language} strings:"), String::new()];
        for (text, file, line) in entries {
            if !file.is_empty() {
                lines.push(format!("    # game/{file}:{line}"));
            }
            lines.extend([format!("    old \"{text}\""), format!("    new \"{text}\""), String::new()]);
        }
        let nl = if old_text.contains("\r\n") { "\r\n" } else { "\n" };
        let block = lines.join("\n");
        let head = if py_strip(&old_text).is_empty() { String::new() } else { format!("{}\n\n", old_text.trim_end_matches(['\r', '\n'])) };
        atomic_write_text(path, &format!("{head}{block}"), Some(nl), backup_dir, clock)?;
        files.push((path.strip_prefix(&tl_dir).unwrap_or(path).to_string_lossy().replace('\\', "/"), entries.len()));
    }
    let added = files.iter().map(|f| f.1).sum();
    Ok(Written { file: tl_dir.join(FILE_NAME).to_string_lossy().into_owned(), files, added, skipped })
}

/// Functions of the game that are called again and again with a text as their first argument (`notify("Item added")`): the translator may add them to the project's list of
/// functions that show text. The most called first: (name, calls, example).
pub fn suggest_functions(game_dir: &Path, known: &HashSet<String>, functions: &[String], ignored: &[String], min_calls: usize) -> Vec<(String, usize, String)> {
    let have: HashSet<&String> = functions.iter().collect();
    let skip_texts: HashSet<&String> = ignored.iter().collect();
    let mut count: IndexMap<String, usize> = IndexMap::new();
    let mut example: IndexMap<String, String> = IndexMap::new();
    for (_rel, path) in scripts(game_dir) {
        let Some(lines) = read_lines(&path, true) else { continue };
        for line in lines {
            let s = py_strip(&line);
            if s.is_empty() || s.starts_with('#') || STATEMENT_SKIP.iter().any(|b| s.starts_with(b)) || p().character_def.is_match(&line) {
                continue;
            }
            for m in p().call.captures_iter(&line) {
                let name = m[1].to_string();
                if name.contains('.') || LIST_METHODS.contains(&name.as_str()) {
                    continue; // methods of objects and lists are not "functions of the game"
                }
                if have.contains(&name) || NOT_TEXT_FUNCTIONS.contains(&name.as_str()) || p().noise_name.is_match(&name) {
                    continue;
                }
                let whole = m.get(0).unwrap();
                let after = whole.start() + whole.as_str().find('(').unwrap() + 1;
                let Some(lit) = p().literal.captures(line[after..].trim_start_matches(crate::util::is_space)).filter(|c| c.get(0).unwrap().start() == 0) else {
                    continue;
                };
                let text = literal(&lit);
                if known.contains(&text) || skip_texts.contains(&text) || !looks_like_text(&text) || !py_strip(&text).contains(' ') {
                    continue;
                }
                *count.entry(name.clone()).or_insert(0) += 1;
                example.entry(name).or_insert(text);
            }
        }
    }
    let mut out: Vec<(String, usize, String)> = count.into_iter().filter(|(_, c)| *c >= min_calls).map(|(n, c)| (n.clone(), c, example[&n].clone())).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.truncate(20);
    out
}

pub const SETTINGS_FILE: &str = "missed.json";

/// `{functions: [...], ignored: [...]}` of the project (`<data dir>/missed.json`); a missing or damaged file means the defaults.
pub fn load_settings(data_dir: &Path) -> (Vec<String>, Vec<String>) {
    let raw: Value = std::fs::read(data_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b)).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let strings = |key: &str| -> BTreeSet<String> {
        match raw.get(key).and_then(Value::as_array) {
            Some(a) => a.iter().filter_map(|x| x.as_str()).filter(|s| !s.is_empty()).map(String::from).collect(),
            None => BTreeSet::new(),
        }
    };
    (strings("functions").into_iter().filter(|f| p().func_name.is_match(f)).collect(), strings("ignored").into_iter().collect())
}

/// Write the lists (sorted, without duplicates); returns what was saved.
pub fn save_settings(
    data_dir: &Path,
    functions: &[String],
    ignored: &[String],
    clock: &dyn crate::clock::Clock,
) -> Result<(Vec<String>, Vec<String>), WriteFailed> {
    let funcs: BTreeSet<String> = functions.iter().map(|f| py_strip(f).to_string()).filter(|f| p().func_name.is_match(f)).collect();
    let ign: BTreeSet<String> = ignored.iter().filter(|t| !t.is_empty()).cloned().collect();
    let mut clean = Map::new();
    clean.insert("version".into(), json!(1));
    clean.insert("functions".into(), json!(funcs));
    clean.insert("ignored".into(), json!(ign));
    let text = crate::pyfmt::json_dumps_indent1(&Value::Object(clean)) + "\n";
    atomic_write_text(&data_dir.join(SETTINGS_FILE), &text, None, None, clock)?;
    Ok((funcs.into_iter().collect(), ign.into_iter().collect()))
}
